//! Export functionality for blocks data.

#![allow(dead_code)]

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
#[allow(dead_code)]
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
        writeln!(
            writer,
            "id,command,directory,created_at,modified_at,exit_code,output_preview"
        )?;

        // Write each block as a CSV row
        for block in blocks {
            let output_preview = block
                .output
                .chars()
                .take(100)
                .collect::<String>()
                .replace('\n', "\\n");
            writeln!(
                writer,
                "{},{},{},{},{},{:?},\"{}\"",
                block.id,
                block.command.replace(',', "\\,"),
                block.directory.display(),
                block.created_at.to_rfc3339(),
                block.modified_at.to_rfc3339(),
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
#[derive(Debug)]
pub struct ExportManager {
    // Add fields as needed
}

impl Default for ExportManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportManager {
    pub fn new() -> Self {
        Self {}
    }

    pub fn create_exporter<'a>(&self, storage: &'a BlockStorage) -> BlockExporter<'a> {
        BlockExporter::new(storage)
    }

    /// Export a list of blocks into bytes in the specified format.
    pub async fn export(
        &self,
        blocks: Vec<std::sync::Arc<Block>>,
        format: ExportFormat,
        _options: ExportOptions,
    ) -> anyhow::Result<Vec<u8>> {
        let blocks_owned: Vec<Block> = blocks.into_iter().map(|b| (*b).clone()).collect();
        let mut buf: Vec<u8> = Vec::new();
        match format {
            ExportFormat::Json => {
                buf = serde_json::to_vec_pretty(&blocks_owned)?;
            }
            ExportFormat::Yaml => {
                let s = serde_yaml::to_string(&blocks_owned)?;
                buf.extend_from_slice(s.as_bytes());
            }
            ExportFormat::Csv => {
                // Write CSV into the buffer using the same logic as BlockExporter.
                self.write_csv(&mut buf, &blocks_owned)?;
            }
        }
        Ok(buf)
    }

    fn write_csv(&self, writer: &mut Vec<u8>, blocks: &[Block]) -> anyhow::Result<()> {
        use std::io::Write as _;
        writeln!(
            writer,
            "id,command,directory,created_at,modified_at,exit_code,output_preview"
        )?;
        for block in blocks {
            let output_preview = block
                .output
                .chars()
                .take(100)
                .collect::<String>()
                .replace('\n', "\\n");
            writeln!(
                writer,
                "{},{},{},{},{},{:?},\"{}\"",
                block.id,
                block.command.replace(',', "\\,"),
                block.directory.display(),
                block.created_at.to_rfc3339(),
                block.modified_at.to_rfc3339(),
                block.exit_code,
                output_preview
            )?;
        }
        Ok(())
    }

    /// Import blocks from bytes in the specified format.
    pub async fn import(
        &self,
        data: &[u8],
        format: ExportFormat,
        _options: &ImportOptions,
    ) -> anyhow::Result<Vec<Block>> {
        let blocks = match format {
            ExportFormat::Json => serde_json::from_slice::<Vec<Block>>(data)?,
            ExportFormat::Yaml => serde_yaml::from_slice::<Vec<Block>>(data)?,
            ExportFormat::Csv => {
                // Very basic CSV importer: expects the header written by export.
                let s = std::str::from_utf8(data)?;
                let mut lines = s.lines();
                // skip header
                let _ = lines.next();
                let mut out = Vec::new();
                for line in lines {
                    // Split on commas not handling escapes fully; for robust CSV use a csv crate.
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() < 7 {
                        continue;
                    }
                    let id = parts[0].trim();
                    let command = parts[1].replace("\\,", ",");
                    let directory = std::path::PathBuf::from(parts[2]);
                    let created_at =
                        chrono::DateTime::parse_from_rfc3339(parts[3])?.with_timezone(&chrono::Utc);
                    let modified_at =
                        chrono::DateTime::parse_from_rfc3339(parts[4])?.with_timezone(&chrono::Utc);
                    let exit_code = parts[5].parse::<i32>().ok();
                    let output_preview = parts[6].trim().trim_matches('"').replace("\\n", "\n");
                    out.push(Block {
                        id: super::BlockId::from_string(id)?,
                        command,
                        output: output_preview,
                        directory,
                        environment: std::collections::HashMap::new(),
                        shell: super::ShellType::Bash,
                        created_at,
                        modified_at,
                        tags: std::collections::HashSet::new(),
                        starred: false,
                        parent_id: None,
                        children: Vec::new(),
                        metadata: super::BlockMetadata::default(),
                        status: super::ExecutionStatus::Success,
                        exit_code,
                        duration_ms: None,
                    });
                }
                out
            }
        };
        Ok(blocks)
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
    /// When true, new IDs are generated for imported blocks.
    pub generate_new_ids: bool,
}

impl ImportOptions {
    pub fn new(source_path: std::path::PathBuf, format: ExportFormat) -> Self {
        Self {
            source_path,
            format,
            overwrite_existing: false,
            generate_new_ids: false,
        }
    }
}
