//! Export functionality for blocks data.

use std::io::Write;

use super::Block;
use crate::blocks_v2::storage::BlockStorage;

/// Format for exporting blocks data.
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Json,
    Csv,
    Yaml,
}

/// Export blocks to different formats.
pub struct BlockExporter<'a> {
    storage: &'a BlockStorage,
}

impl<'a> BlockExporter<'a> {
    pub fn new(storage: &'a BlockStorage) -> Self {
        Self { storage }
    }

    /// Export all blocks to the specified writer in the given format.
    pub async fn export_all<W: Write>(
        &self,
        writer: &mut W,
        format: ExportFormat,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let blocks = self.storage.get_all_blocks().await?;
        self.export_blocks(writer, &blocks, format)
    }

    /// Export specific blocks to the specified writer in the given format.
    pub fn export_blocks<W: Write>(
        &self,
        writer: &mut W,
        blocks: &[Block],
        format: ExportFormat,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match format {
            ExportFormat::Json => self.export_json(writer, blocks),
            ExportFormat::Csv => self.export_csv(writer, blocks),
            ExportFormat::Yaml => self.export_yaml(writer, blocks),
        }
    }

    fn export_json<W: Write>(
        &self,
        writer: &mut W,
        blocks: &[Block],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(blocks)?;
        writer.write_all(json.as_bytes())?;
        Ok(())
    }

    fn export_csv<W: Write>(
        &self,
        writer: &mut W,
        blocks: &[Block],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Write CSV header
        writeln!(writer, "id,command,working_dir,start_time,end_time,exit_code,output_preview")?;

        // Write each block as a CSV row
        for block in blocks {
            let output_preview = block.output.chars().take(100).collect::<String>().replace('\n', "\\n");
            writeln!(
                writer,
                "{},{},{},{:?},{:?},{:?},\"{}\"",
                block.id,
                block.command.replace(',', "\\,"),
                block.working_dir.display(),
                block.start_time,
                block.end_time,
                block.exit_code,
                output_preview
            )?;
        }
        Ok(())
    }

    fn export_yaml<W: Write>(
        &self,
        writer: &mut W,
        blocks: &[Block],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let yaml = serde_yaml::to_string(blocks)?;
        writer.write_all(yaml.as_bytes())?;
        Ok(())
    }
}

/// Export manager for handling various export operations
pub struct ExportManager {
    // Add fields as needed
}

impl ExportManager {
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn create_exporter<'a>(&self, storage: &'a BlockStorage) -> BlockExporter<'a> {
        BlockExporter::new(storage)
    }
}

/// Options for exporting blocks
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub output_path: Option<std::path::PathBuf>,
    pub filter_command: Option<String>,
    #[cfg(feature = "blocks")]
    pub date_range: Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>,
    #[cfg(not(feature = "blocks"))]
    pub date_range: Option<()>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Json,
            output_path: None,
            filter_command: None,
            date_range: None,
        }
    }
}

/// Options for importing blocks
#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub source_path: std::path::PathBuf,
    pub format: ExportFormat,
    pub overwrite_existing: bool,
}

impl ImportOptions {
    pub fn new(source_path: std::path::PathBuf, format: ExportFormat) -> Self {
        Self {
            source_path,
            format,
            overwrite_existing: false,
        }
    }
}
