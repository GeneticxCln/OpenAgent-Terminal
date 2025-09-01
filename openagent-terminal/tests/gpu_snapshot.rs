// GPU Snapshot Testing Infrastructure
// Visual regression detection for terminal rendering

use image::{DynamicImage, ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    pub name: String,
    pub platforms: Vec<String>,
    pub gpu_types: Vec<String>,
    pub compare_threshold: f64,
    pub dimensions: (u32, u32),
}

#[derive(Debug)]
pub struct SnapshotTest {
    config: SnapshotConfig,
    golden_path: PathBuf,
    output_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub passed: bool,
    pub similarity: f64,
    pub diff_pixels: usize,
    pub max_difference: u8,
    pub diff_image: Option<DynamicImage>,
}

impl SnapshotTest {
    pub fn new(config: SnapshotConfig) -> Self {
        let golden_path = PathBuf::from("tests/golden_images");
        let output_path = PathBuf::from("tests/snapshot_output");

        // Create directories if they don't exist
        fs::create_dir_all(&golden_path).ok();
        fs::create_dir_all(&output_path).ok();

        Self { config, golden_path, output_path }
    }

    /// Capture a snapshot of the current terminal render
    pub fn capture_snapshot(
        &self,
        renderer: &impl TerminalRenderer,
    ) -> Result<DynamicImage, String> {
        // Get framebuffer from renderer
        let framebuffer =
            renderer.get_framebuffer().map_err(|e| format!("Failed to get framebuffer: {}", e))?;

        // Convert to image
        let (width, height) = self.config.dimensions;
        let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, framebuffer)
            .ok_or("Failed to create image from framebuffer")?;

        Ok(DynamicImage::ImageRgba8(image))
    }

    /// Compare snapshot against golden image
    pub fn compare_with_golden(&self, snapshot: &DynamicImage) -> ComparisonResult {
        let golden_file = self.get_golden_path();

        // Load golden image
        let golden = match image::open(&golden_file) {
            Ok(img) => img,
            Err(_) => {
                // No golden image exists, this is the first run
                return ComparisonResult {
                    passed: false,
                    similarity: 0.0,
                    diff_pixels: 0,
                    max_difference: 0,
                    diff_image: None,
                };
            },
        };

        self.compare_images(&golden, snapshot)
    }

    /// Compare two images pixel by pixel
    fn compare_images(&self, golden: &DynamicImage, snapshot: &DynamicImage) -> ComparisonResult {
        let golden_rgba = golden.to_rgba8();
        let snapshot_rgba = snapshot.to_rgba8();

        // Check dimensions match
        if golden_rgba.dimensions() != snapshot_rgba.dimensions() {
            return ComparisonResult {
                passed: false,
                similarity: 0.0,
                diff_pixels: usize::MAX,
                max_difference: 255,
                diff_image: None,
            };
        }

        let (width, height) = golden_rgba.dimensions();
        let mut diff_image = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height);
        let mut diff_pixels = 0;
        let mut max_difference = 0u8;
        let mut total_difference = 0u64;

        for (x, y, golden_pixel) in golden_rgba.enumerate_pixels() {
            let snapshot_pixel = snapshot_rgba.get_pixel(x, y);

            // Calculate per-channel differences
            let mut pixel_diff = 0u32;
            let mut diff_color = Rgba([0, 0, 0, 255]);

            for i in 0..4 {
                let diff = (golden_pixel[i] as i32 - snapshot_pixel[i] as i32).abs() as u8;
                pixel_diff += diff as u32;
                max_difference = max_difference.max(diff);

                // Highlight differences in red
                if diff > 0 {
                    diff_color[0] = 255;
                    diff_color[1] = (255 - diff).min(128);
                    diff_color[2] = (255 - diff).min(128);
                }
            }

            if pixel_diff > 0 {
                diff_pixels += 1;
                total_difference += pixel_diff as u64;
            }

            diff_image.put_pixel(x, y, diff_color);
        }

        let total_pixels = (width * height) as f64;
        let similarity = 1.0 - (diff_pixels as f64 / total_pixels);
        let passed = similarity >= self.config.compare_threshold;

        ComparisonResult {
            passed,
            similarity,
            diff_pixels,
            max_difference,
            diff_image: Some(DynamicImage::ImageRgba8(diff_image)),
        }
    }

    /// Update golden image with current snapshot
    pub fn update_golden(&self, snapshot: &DynamicImage) -> Result<(), String> {
        let golden_file = self.get_golden_path();
        snapshot.save(&golden_file).map_err(|e| format!("Failed to save golden image: {}", e))
    }

    /// Save comparison results
    pub fn save_results(
        &self,
        result: &ComparisonResult,
        snapshot: &DynamicImage,
    ) -> Result<(), String> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let result_dir = self.output_path.join(format!("{}_{}", self.config.name, timestamp));
        fs::create_dir_all(&result_dir)
            .map_err(|e| format!("Failed to create result directory: {}", e))?;

        // Save snapshot
        let snapshot_file = result_dir.join("snapshot.png");
        snapshot.save(&snapshot_file).map_err(|e| format!("Failed to save snapshot: {}", e))?;

        // Save diff image if it exists
        if let Some(ref diff_image) = result.diff_image {
            let diff_file = result_dir.join("diff.png");
            diff_image.save(&diff_file).map_err(|e| format!("Failed to save diff image: {}", e))?;
        }

        // Save result metadata
        let metadata = serde_json::json!({
            "test_name": self.config.name,
            "passed": result.passed,
            "similarity": result.similarity,
            "diff_pixels": result.diff_pixels,
            "max_difference": result.max_difference,
            "threshold": self.config.compare_threshold,
            "timestamp": timestamp.to_string(),
        });

        let metadata_file = result_dir.join("result.json");
        fs::write(&metadata_file, serde_json::to_string_pretty(&metadata).unwrap())
            .map_err(|e| format!("Failed to save metadata: {}", e))?;

        Ok(())
    }

    fn get_golden_path(&self) -> PathBuf {
        let platform = std::env::consts::OS;
        let filename = format!("{}_{}.png", self.config.name, platform);
        self.golden_path.join(filename)
    }
}

// Trait for terminal renderers to implement
pub trait TerminalRenderer {
    fn get_framebuffer(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
    fn get_dimensions(&self) -> (u32, u32);
}

// Test runner for multiple snapshot tests
pub struct SnapshotTestRunner {
    tests: Vec<SnapshotTest>,
    update_golden: bool,
}

impl SnapshotTestRunner {
    pub fn new(update_golden: bool) -> Self {
        Self { tests: Vec::new(), update_golden }
    }

    pub fn add_test(&mut self, config: SnapshotConfig) {
        self.tests.push(SnapshotTest::new(config));
    }

    pub fn run_all(&self, renderer: &impl TerminalRenderer) -> Vec<(String, ComparisonResult)> {
        let mut results = Vec::new();

        for test in &self.tests {
            let snapshot = match test.capture_snapshot(renderer) {
                Ok(img) => img,
                Err(e) => {
                    eprintln!("Failed to capture snapshot for {}: {}", test.config.name, e);
                    results.push((
                        test.config.name.clone(),
                        ComparisonResult {
                            passed: false,
                            similarity: 0.0,
                            diff_pixels: 0,
                            max_difference: 0,
                            diff_image: None,
                        },
                    ));
                    continue;
                },
            };

            if self.update_golden {
                if let Err(e) = test.update_golden(&snapshot) {
                    eprintln!("Failed to update golden for {}: {}", test.config.name, e);
                }
                results.push((
                    test.config.name.clone(),
                    ComparisonResult {
                        passed: true,
                        similarity: 1.0,
                        diff_pixels: 0,
                        max_difference: 0,
                        diff_image: None,
                    },
                ));
            } else {
                let result = test.compare_with_golden(&snapshot);

                // Save results for failed tests
                if !result.passed {
                    if let Err(e) = test.save_results(&result, &snapshot) {
                        eprintln!("Failed to save results for {}: {}", test.config.name, e);
                    }
                }

                results.push((test.config.name.clone(), result));
            }
        }

        results
    }

    pub fn print_summary(&self, results: &[(String, ComparisonResult)]) {
        println!("\n=== Snapshot Test Summary ===");
        let total = results.len();
        let passed = results.iter().filter(|(_, r)| r.passed).count();
        let failed = total - passed;

        println!("Total: {} | Passed: {} | Failed: {}", total, passed, failed);

        if failed > 0 {
            println!("\nFailed tests:");
            for (name, result) in results.iter().filter(|(_, r)| !r.passed) {
                println!("  ❌ {} (similarity: {:.2}%)", name, result.similarity * 100.0);
            }
        }

        if passed > 0 {
            println!("\nPassed tests:");
            for (name, result) in results.iter().filter(|(_, r)| r.passed) {
                println!("  ✅ {} (similarity: {:.2}%)", name, result.similarity * 100.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRenderer {
        framebuffer: Vec<u8>,
        dimensions: (u32, u32),
    }

    impl TerminalRenderer for MockRenderer {
        fn get_framebuffer(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            Ok(self.framebuffer.clone())
        }

        fn get_dimensions(&self) -> (u32, u32) {
            self.dimensions
        }
    }

    #[test]
    fn test_snapshot_capture() {
        let config = SnapshotConfig {
            name: "test_capture".to_string(),
            platforms: vec!["linux".to_string()],
            gpu_types: vec!["mock".to_string()],
            compare_threshold: 0.99,
            dimensions: (100, 50),
        };

        let test = SnapshotTest::new(config);

        // Create mock renderer with test pattern
        let mut framebuffer = vec![0u8; 100 * 50 * 4];
        for i in 0..framebuffer.len() / 4 {
            framebuffer[i * 4] = (i % 256) as u8; // R
            framebuffer[i * 4 + 1] = ((i * 2) % 256) as u8; // G
            framebuffer[i * 4 + 2] = ((i * 3) % 256) as u8; // B
            framebuffer[i * 4 + 3] = 255; // A
        }

        let renderer = MockRenderer { framebuffer, dimensions: (100, 50) };

        let snapshot = test.capture_snapshot(&renderer).unwrap();
        assert_eq!(snapshot.dimensions(), (100, 50));
    }

    #[test]
    fn test_image_comparison() {
        let config = SnapshotConfig {
            name: "test_compare".to_string(),
            platforms: vec!["linux".to_string()],
            gpu_types: vec!["mock".to_string()],
            compare_threshold: 0.95,
            dimensions: (10, 10),
        };

        let test = SnapshotTest::new(config);

        // Create two slightly different images
        let img1 = DynamicImage::new_rgba8(10, 10);
        let mut img2 = DynamicImage::new_rgba8(10, 10);

        // Modify one pixel
        img2.as_mut_rgba8().unwrap().put_pixel(5, 5, Rgba([255, 0, 0, 255]));

        let result = test.compare_images(&img1, &img2);
        assert!(!result.passed); // Should fail due to difference
        assert_eq!(result.diff_pixels, 1);
    }
}
