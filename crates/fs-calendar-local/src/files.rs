//! Where received and written files live (fs-files at the border): a small trait and a memory implementation.
//!
//! Pattern: Adapter port. A `FileRef` carries the checksum, so a read can be checked against what was promised. The
//! memory implementation is the layer's (`fs_objects::v5::files`); this file only maps the trait's file types.

use async_trait::async_trait;
use fs_calendar_testdata::draft::{FileRef, MediaType, Result};
use fs_objects::v5::files::{self as layer, FileStore as _};
use fs_types as ft;

use crate::bridge::{adapt, reference, reference_back};

/// Read and write files by reference.
#[async_trait]
pub trait FileStore: Send + Sync {
    /// The bytes of a file.
    async fn read(&self, f: &FileRef) -> Result<Vec<u8>>;
    /// Keep bytes; the reference carries their checksum.
    async fn write(&self, bytes: Vec<u8>, media: &MediaType) -> Result<FileRef>;
}

/// Files in memory (tests, and the CLI before fs-files is wired in).
#[derive(Default)]
pub struct MemoryFiles(layer::MemoryFiles);

impl MemoryFiles {
    /// Put bytes in and get the reference (`urn:sha256:<hex>`).
    ///
    /// # Errors
    /// `Unavailable` when the reference cannot be built (never in practice: it is made from hex).
    pub fn put(&self, bytes: &[u8], media: &MediaType) -> Result<FileRef> {
        let f = self
            .0
            .put(bytes, &ft::MediaType(media.0.clone()))
            .map_err(adapt)?;
        Ok(FileRef {
            iri: reference_back(&f.iri)?,
            sha256: f.sha256,
            media_type: MediaType(f.media_type.0),
        })
    }
}

#[async_trait]
impl FileStore for MemoryFiles {
    async fn read(&self, f: &FileRef) -> Result<Vec<u8>> {
        let f = ft::FileRef {
            iri: reference(&f.iri)?,
            sha256: f.sha256,
            media_type: ft::MediaType(f.media_type.0.clone()),
        };
        self.0.read(&f).await.map_err(adapt)
    }

    async fn write(&self, bytes: Vec<u8>, media: &MediaType) -> Result<FileRef> {
        self.put(&bytes, media)
    }
}
