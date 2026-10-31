//! The `.kernsetzer` project file: a zip archive with
//!
//! ```text
//! meta.json          { "format_version": 2, "created_with": "kernsetzer 0.1.0", "modified": "…" }
//! book.json          the Book, pretty-printed
//! assets/<id>        images, referenced by id from book.json
//! ```
//!
//! plus `book.json` (de)serialization with schema migrations, and the
//! autosave sidecar `<name>.kernsetzer.autosave` next to the project.
//!
//! Saving writes a temporary sibling and renames it over the project, so a
//! crash mid-save never leaves a half-written project behind.

use crate::asset::{Asset, ImageFormat};
use crate::{Book, FORMAT_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

const BOOK_JSON: &str = "book.json";
const META_JSON: &str = "meta.json";
const ASSETS_DIR: &str = "assets/";

/// Largest `book.json` / `meta.json` accepted (a 100k-word novel is ~2 MB).
pub const MAX_JSON_BYTES: u64 = 256 * 1024 * 1024;
/// Largest single asset accepted (same limit as the .docx importer).
pub const MAX_ASSET_BYTES: u64 = 100 * 1024 * 1024;

/// The project file's extension, without the dot.
pub const EXTENSION: &str = "kernsetzer";
/// The extension of project files from before the rename. Files with it
/// are still opened.
pub const OLD_EXTENSION: &str = "setzer";

/// The build that writes a file, as recorded in `meta.json`.
pub const CREATED_WITH: &str = concat!("kernsetzer ", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("book.json has no valid format_version")]
    MissingVersion,
    #[error(
        "this project was saved by a newer Kernsetzer (format {found}, this version reads up to {max}). Update Kernsetzer to open it.",
        max = FORMAT_VERSION
    )]
    TooNew { found: u64 },
    #[error("this is not a Kernsetzer project (no book.json inside)")]
    NotAProject,
    #[error("the project file is damaged: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{name} in the project is larger than {limit} bytes")]
    TooLarge { name: String, limit: u64 },
    #[error("\"{0}\" cannot be used as an image file name in a project")]
    InvalidAssetId(String),
    #[error("two images in the project are both named \"{0}\"")]
    DuplicateAsset(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Contents of `meta.json`: facts about the file, not about the book.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    /// Container format; always written as [`FORMAT_VERSION`].
    pub format_version: u32,
    /// The build that created the project, e.g. `kernsetzer 0.1.0`. Kept
    /// across saves (useful in support requests).
    #[serde(default)]
    pub created_with: String,
    /// When the file was last saved, RFC 3339 in UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    /// Fields written by newer builds, kept on round-trip.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Meta {
    /// Meta for a project created by this build.
    pub fn new() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            created_with: CREATED_WITH.into(),
            modified: None,
            extra: Map::new(),
        }
    }
}

impl Default for Meta {
    fn default() -> Self {
        Self::new()
    }
}

/// A book with its images and file metadata: what a `.kernsetzer` holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub book: Book,
    /// In file order; ids are unique and valid ([`Asset::is_valid_id`]).
    pub assets: Vec<Asset>,
    pub meta: Meta,
}

impl Project {
    pub fn new(book: Book, assets: Vec<Asset>) -> Self {
        Self {
            book,
            assets,
            meta: Meta::new(),
        }
    }

    pub fn asset(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }

    /// Read a `.kernsetzer` archive. `book.json` is migrated to the current
    /// format. A missing or unreadable `meta.json` is not an error (the
    /// book is what matters); entries under `assets/` whose names are not
    /// valid ids (`.DS_Store`, sub-folders) and unknown entries are
    /// skipped. Asset metadata is derived from the bytes.
    pub fn read<R: Read + Seek>(reader: R) -> Result<Self, ProjectError> {
        let mut zip = ZipArchive::new(reader)?;
        let mut book = None;
        let mut meta = None;
        let mut assets: Vec<Asset> = Vec::new();
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            if file.is_dir() {
                continue;
            }
            let name = file.name()?.into_owned();
            if name == BOOK_JSON {
                let bytes = read_limited(&mut file, &name, MAX_JSON_BYTES)?;
                book = Some(book_from_json(&String::from_utf8_lossy(&bytes))?);
            } else if name == META_JSON {
                let bytes = read_limited(&mut file, &name, MAX_JSON_BYTES)?;
                meta = serde_json::from_slice::<Meta>(&bytes).ok();
            } else if let Some(id) = name.strip_prefix(ASSETS_DIR) {
                if !Asset::is_valid_id(id) || assets.iter().any(|a| a.id == id) {
                    continue;
                }
                let id = id.to_owned();
                let bytes = read_limited(&mut file, &name, MAX_ASSET_BYTES)?;
                assets.push(Asset::new(id, bytes));
            }
        }
        let meta = meta.unwrap_or_else(|| Meta {
            created_with: "unknown".into(),
            ..Meta::new()
        });
        if meta.format_version > FORMAT_VERSION {
            return Err(ProjectError::TooNew {
                found: meta.format_version.into(),
            });
        }
        Ok(Self {
            book: book.ok_or(ProjectError::NotAProject)?,
            assets,
            meta,
        })
    }

    /// Write the archive as is (`meta.modified` unchanged).
    pub fn write<W: Write + Seek>(&self, writer: W) -> Result<W, ProjectError> {
        self.write_with(writer, self.meta.modified.as_deref())
    }

    fn write_with<W: Write + Seek>(
        &self,
        writer: W,
        modified: Option<&str>,
    ) -> Result<W, ProjectError> {
        let mut seen: Vec<&str> = Vec::with_capacity(self.assets.len());
        for a in &self.assets {
            if !Asset::is_valid_id(&a.id) {
                return Err(ProjectError::InvalidAssetId(a.id.clone()));
            }
            if seen.contains(&a.id.as_str()) {
                return Err(ProjectError::DuplicateAsset(a.id.clone()));
            }
            seen.push(&a.id);
        }
        let meta = Meta {
            format_version: FORMAT_VERSION,
            modified: modified.map(str::to_owned),
            ..self.meta.clone()
        };

        // Fixed timestamps keep the archive byte-identical for equal input;
        // the real save time is in meta.json.
        let opts = SimpleFileOptions::default().last_modified_time(DateTime::DEFAULT);
        let deflated = opts.compression_method(CompressionMethod::Deflated);
        let stored = opts.compression_method(CompressionMethod::Stored);
        let mut zip = ZipWriter::new(writer);
        zip.start_file(META_JSON, deflated)?;
        zip.write_all(serde_json::to_string_pretty(&meta)?.as_bytes())?;
        zip.start_file(BOOK_JSON, deflated)?;
        zip.write_all(book_to_json(&self.book)?.as_bytes())?;
        for a in &self.assets {
            // PNG, JPEG, GIF and WebP are compressed already.
            let method = match ImageFormat::sniff(&a.bytes) {
                ImageFormat::Supported { ext, .. } if ext != "svg" => stored,
                _ => deflated,
            };
            zip.start_file(format!("{ASSETS_DIR}{}", a.id), method)?;
            zip.write_all(&a.bytes)?;
        }
        Ok(zip.finish()?)
    }

    pub fn open(path: &Path) -> Result<Self, ProjectError> {
        Self::read(BufReader::new(File::open(path)?))
    }

    /// Save to `path` (atomically), set `meta.modified`, and remove the
    /// autosave sidecar: after a clean save there is nothing to recover.
    pub fn save(&mut self, path: &Path) -> Result<(), ProjectError> {
        let now = now_rfc3339();
        write_atomically(path, |f| self.write_with(f, Some(&now)).map(drop))?;
        self.meta.modified = Some(now);
        discard_autosave(path)
    }

    /// Write the autosave sidecar for the project at `path`
    /// ([`autosave_path`]); the project file itself is not touched.
    pub fn autosave(&self, path: &Path) -> Result<(), ProjectError> {
        let now = now_rfc3339();
        write_atomically(&autosave_path(path), |f| {
            self.write_with(f, Some(&now)).map(drop)
        })
    }
}

/// The autosave sidecar for a project: `Roman.kernsetzer` →
/// `Roman.kernsetzer.autosave`, in the same folder. If it exists when a
/// project is opened, the last session ended without a clean save.
pub fn autosave_path(project: &Path) -> PathBuf {
    with_suffix(project, ".autosave")
}

/// Remove the autosave sidecar, if there is one.
pub fn discard_autosave(project: &Path) -> Result<(), ProjectError> {
    match fs::remove_file(autosave_path(project)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = OsString::from(path.as_os_str());
    s.push(suffix);
    PathBuf::from(s)
}

/// Write `path` via a temporary sibling that is synced and renamed over
/// it, so readers see the old file or the new one, never a partial one.
fn write_atomically(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<(), ProjectError>,
) -> Result<(), ProjectError> {
    let tmp = with_suffix(path, ".saving");
    let result = (|| {
        let mut out = BufWriter::new(File::create(&tmp)?);
        write(&mut out)?;
        out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn read_limited(file: &mut impl Read, name: &str, limit: u64) -> Result<Vec<u8>, ProjectError> {
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(ProjectError::TooLarge {
            name: name.into(),
            limit,
        });
    }
    Ok(bytes)
}

/// The current time as RFC 3339 in UTC, to the second.
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339(secs)
}

fn rfc3339(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    // Civil date from days since 1970-01-01 (H. Hinnant, `civil_from_days`).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// Parse `book.json`, migrating older formats to the current one.
pub fn book_from_json(s: &str) -> Result<Book, ProjectError> {
    let v: Value = serde_json::from_str(s)?;
    Ok(serde_json::from_value(migrate(v)?)?)
}

/// One schema step: `MIGRATIONS[i]` turns a version `i + 1` `book.json`
/// into version `i + 2`. Each step is a plain function on JSON, written
/// once and never changed afterwards (old files stay old). When the
/// schema changes incompatibly: bump [`FORMAT_VERSION`], append the step,
/// and commit a fixture of the old version under `tests/fixtures/`.
pub type Migration = fn(Value) -> Result<Value, ProjectError>;

pub const MIGRATIONS: &[Migration] = &[migrate_v1_to_v2];

/// Version 2 added fields only (picture size and placement, and the table
/// block), each optional and skipped when default, so a version 1
/// `book.json` is already a valid version 2 one. The bump exists so that a
/// version 1 build refuses a file that may use them instead of drawing
/// a bleed or beside picture as a centred one.
fn migrate_v1_to_v2(v: Value) -> Result<Value, ProjectError> {
    Ok(v)
}

const _: () = assert!(MIGRATIONS.len() + 1 == FORMAT_VERSION as usize);

/// Bring a `book.json` value of any supported version to the current
/// format, applying the migrations in order.
pub fn migrate(v: Value) -> Result<Value, ProjectError> {
    migrate_with(v, MIGRATIONS)
}

fn migrate_with(mut v: Value, steps: &[Migration]) -> Result<Value, ProjectError> {
    let current = steps.len() as u64 + 1;
    let version = v
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or(ProjectError::MissingVersion)?;
    if version == 0 {
        return Err(ProjectError::MissingVersion);
    }
    if version > current {
        return Err(ProjectError::TooNew { found: version });
    }
    for step in steps.iter().skip((version - 1) as usize) {
        v = step(v)?;
    }
    match v.as_object_mut() {
        Some(obj) => {
            obj.insert("format_version".into(), Value::from(current));
            Ok(v)
        }
        None => Err(ProjectError::MissingVersion),
    }
}

pub fn book_to_json(book: &Book) -> Result<String, ProjectError> {
    Ok(serde_json::to_string_pretty(book)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::testdata::png_3x2_300dpi;
    use crate::{Block, Metadata, Section};
    use std::io::Cursor;

    fn book() -> Book {
        let mut b = Book::new(Metadata {
            title: "T".into(),
            ..Default::default()
        });
        b.sections.push(Section::chapter(
            "Eins",
            vec![Block::image("map.png", "Karte", None)],
        ));
        b
    }

    fn project() -> Project {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>"#;
        Project::new(
            book(),
            vec![
                Asset::new("map.png", png_3x2_300dpi()),
                Asset::new("ornament.svg", svg.to_vec()),
            ],
        )
    }

    fn to_bytes(p: &Project) -> Vec<u8> {
        p.write(Cursor::new(Vec::new())).unwrap().into_inner()
    }

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn round_trip() {
        let b = book();
        let s = book_to_json(&b).unwrap();
        assert_eq!(book_from_json(&s).unwrap(), b);
    }

    #[test]
    fn rejects_newer() {
        let err = book_from_json(r#"{"format_version": 999}"#).unwrap_err();
        assert!(matches!(err, ProjectError::TooNew { found: 999 }));
        let err = book_from_json(r#"{"format_version": 3}"#).unwrap_err();
        assert!(matches!(err, ProjectError::TooNew { found: 3 }));
    }

    #[test]
    fn version_3_project_is_refused() {
        let book = book_to_json(&book())
            .unwrap()
            .replace("\"format_version\": 2", "\"format_version\": 3");
        for meta_version in [2, 3] {
            let meta = format!(r#"{{"format_version":{meta_version},"created_with":"x"}}"#);
            let bytes = zip_of(&[
                ("meta.json", meta.as_bytes()),
                ("book.json", book.as_bytes()),
            ]);
            assert!(matches!(
                Project::read(Cursor::new(bytes)),
                Err(ProjectError::TooNew { found: 3 })
            ));
        }
    }

    #[test]
    fn version_1_book_saves_as_version_2() {
        let v1 = book_to_json(&book())
            .unwrap()
            .replace("\"format_version\": 2", "\"format_version\": 1");
        let b = book_from_json(&v1).unwrap();
        assert_eq!(b.format_version, 2);
        let again = book_to_json(&b).unwrap();
        assert!(again.contains("\"format_version\": 2"));
    }

    #[test]
    fn migrations_run_in_order_from_the_file_version() {
        fn v1_to_v2(mut v: Value) -> Result<Value, ProjectError> {
            v["steps"].as_array_mut().unwrap().push("1→2".into());
            Ok(v)
        }
        fn v2_to_v3(mut v: Value) -> Result<Value, ProjectError> {
            v["steps"].as_array_mut().unwrap().push("2→3".into());
            Ok(v)
        }
        let steps: &[Migration] = &[v1_to_v2, v2_to_v3];
        let run = |version: u64| {
            migrate_with(
                serde_json::json!({ "format_version": version, "steps": [] }),
                steps,
            )
        };
        let v = run(1).unwrap();
        assert_eq!(v["format_version"], 3);
        assert_eq!(v["steps"], serde_json::json!(["1→2", "2→3"]));
        assert_eq!(run(2).unwrap()["steps"], serde_json::json!(["2→3"]));
        assert_eq!(run(3).unwrap()["steps"], serde_json::json!([]));
        assert!(matches!(run(4), Err(ProjectError::TooNew { found: 4 })));
        assert!(matches!(run(0), Err(ProjectError::MissingVersion)));
    }

    #[test]
    fn bad_versions() {
        for json in [
            r#"{}"#,
            r#"[]"#,
            r#"{"format_version": "1"}"#,
            r#"{"format_version": -1}"#,
        ] {
            assert!(
                matches!(book_from_json(json), Err(ProjectError::MissingVersion)),
                "{json}"
            );
        }
    }

    #[test]
    fn archive_round_trip() {
        let p = project();
        let bytes = to_bytes(&p);
        let back = Project::read(Cursor::new(&bytes)).unwrap();
        assert_eq!(back, p);
        assert_eq!(
            back.asset("map.png").unwrap().dpi.map(f32::round),
            Some(300.0)
        );
        // Deterministic: same project, same bytes.
        assert_eq!(to_bytes(&back), bytes);
    }

    #[test]
    fn archive_layout() {
        let bytes = to_bytes(&project());
        let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let names: Vec<_> = zip.file_names().map(|n| n.unwrap().into_owned()).collect();
        assert_eq!(
            names,
            [
                "meta.json",
                "book.json",
                "assets/map.png",
                "assets/ornament.svg"
            ]
        );
        let png = zip.by_name("assets/map.png").unwrap();
        assert_eq!(png.compression(), CompressionMethod::Stored);
        drop(png);
        let mut meta = String::new();
        zip.by_name("meta.json")
            .unwrap()
            .read_to_string(&mut meta)
            .unwrap();
        let meta: Value = serde_json::from_str(&meta).unwrap();
        assert_eq!(meta["format_version"], 2);
        assert_eq!(meta["created_with"], CREATED_WITH);
    }

    #[test]
    fn lenient_read() {
        let book = book_to_json(&book()).unwrap();
        let png = png_3x2_300dpi();
        let bytes = zip_of(&[
            ("book.json", book.as_bytes()),
            ("assets/.DS_Store", b"junk"),
            ("assets/sub/x.png", &png),
            ("__MACOSX/assets/._map.png", b"junk"),
            ("assets/map.png", &png),
            ("thumbnails/cover.png", &png),
        ]);
        let p = Project::read(Cursor::new(bytes)).unwrap();
        assert_eq!(p.meta.created_with, "unknown");
        let ids: Vec<_> = p.assets.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["map.png"]);
        assert_eq!(p.assets[0].width_px, Some(3));
    }

    #[test]
    fn unknown_meta_fields_survive() {
        let book = book_to_json(&book()).unwrap();
        let meta = br#"{"format_version":2,"created_with":"kernsetzer 9","modified":"2030-01-01T00:00:00Z","window":{"zoom":2}}"#;
        let bytes = zip_of(&[("meta.json", meta), ("book.json", book.as_bytes())]);
        let p = Project::read(Cursor::new(bytes)).unwrap();
        assert_eq!(p.meta.created_with, "kernsetzer 9");
        assert_eq!(p.meta.extra["window"]["zoom"], 2);
        let again = Project::read(Cursor::new(to_bytes(&p))).unwrap();
        assert_eq!(again.meta, p.meta);
    }

    #[test]
    fn read_errors() {
        assert!(matches!(
            Project::read(Cursor::new(b"not a zip".to_vec())),
            Err(ProjectError::Zip(_))
        ));
        let bytes = zip_of(&[("assets/map.png", &png_3x2_300dpi())]);
        assert!(matches!(
            Project::read(Cursor::new(bytes)),
            Err(ProjectError::NotAProject)
        ));
        let book = book_to_json(&book()).unwrap();
        let bytes = zip_of(&[
            ("meta.json", br#"{"format_version":7,"created_with":"x"}"#),
            ("book.json", book.as_bytes()),
        ]);
        assert!(matches!(
            Project::read(Cursor::new(bytes)),
            Err(ProjectError::TooNew { found: 7 })
        ));
        // Truncated archives fail cleanly.
        let full = to_bytes(&project());
        for n in (0..full.len()).step_by(7) {
            assert!(Project::read(Cursor::new(&full[..n])).is_err(), "{n}");
        }
    }

    #[test]
    fn damaged_archives_never_panic() {
        let full = to_bytes(&project());
        for i in 0..full.len() {
            for mask in [0x01, 0x80, 0xFF] {
                let mut bytes = full.clone();
                bytes[i] ^= mask;
                let _ = Project::read(Cursor::new(bytes));
            }
        }
    }

    #[test]
    fn write_rejects_bad_ids() {
        let mut p = project();
        p.assets.push(Asset::new("../evil.png", png_3x2_300dpi()));
        assert!(matches!(
            p.write(Cursor::new(Vec::new())),
            Err(ProjectError::InvalidAssetId(_))
        ));
        let mut p = project();
        p.assets.push(Asset::new("map.png", png_3x2_300dpi()));
        assert!(matches!(
            p.write(Cursor::new(Vec::new())),
            Err(ProjectError::DuplicateAsset(_))
        ));
    }

    #[test]
    fn save_open_and_autosave() {
        let dir = std::env::temp_dir().join(format!("setzer-model-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Roman.kernsetzer");
        let sidecar = autosave_path(&path);
        assert_eq!(sidecar, dir.join("Roman.kernsetzer.autosave"));

        let mut p = project();
        p.autosave(&path).unwrap();
        assert!(sidecar.exists() && !path.exists());
        assert_eq!(Project::open(&sidecar).unwrap().book, p.book);

        p.save(&path).unwrap();
        assert!(path.exists() && !sidecar.exists());
        assert!(!with_suffix(&path, ".saving").exists());
        let modified = p.meta.modified.clone().unwrap();
        assert!(
            modified.ends_with('Z') && modified.len() == 20,
            "{modified}"
        );
        assert_eq!(Project::open(&path).unwrap(), p);

        // Saving again replaces the file; a missing sidecar is fine.
        p.book.metadata.title = "Zweiter Titel".into();
        p.save(&path).unwrap();
        assert_eq!(
            Project::open(&path).unwrap().book.metadata.title,
            "Zweiter Titel"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn timestamps() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_792_368_000 + 3_723), "2026-10-19T01:02:03Z");
        assert_eq!(rfc3339(4_102_444_799), "2099-12-31T23:59:59Z");
    }
}
