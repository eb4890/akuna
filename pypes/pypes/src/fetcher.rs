use anyhow::{Context, Result, anyhow};
use sha2::{Sha256, Digest};
use std::path::PathBuf;
use tokio::fs;

pub struct ComponentFetcher {
    client: reqwest::Client,
    cache_dir: PathBuf,
}

impl ComponentFetcher {
    pub fn new() -> Result<Self> {
        let cache_dir = Self::get_cache_dir()?;
        let client = reqwest::Client::builder()
            .user_agent("pypes/0.1.0")
            .build()?;
        Ok(Self { client, cache_dir })
    }

    fn get_cache_dir() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .context("HOME environment variable not set")?;
        let cache = PathBuf::from(home).join(".pypes").join("cache");
        Ok(cache)
    }

    /// Fetch a component from a remote:// URI
    /// Format: remote://registry.example.com/skill-name@version
    pub async fn fetch(&self, uri: &str) -> Result<PathBuf> {
        if !uri.starts_with("remote://") {
            return Err(anyhow!("Invalid remote URI: {}", uri));
        }

        let without_scheme = uri.strip_prefix("remote://").unwrap();
        let parts: Vec<&str> = without_scheme.split('/').collect();
        
        if parts.len() < 2 {
            return Err(anyhow!("Invalid URI format. Expected: remote://host/skill@version"));
        }

        let registry = parts[0];
        let skill_spec = parts[1];
        
        // Parse skill@version
        let skill_parts: Vec<&str> = skill_spec.split('@').collect();
        if skill_parts.len() != 2 {
            return Err(anyhow!("Invalid skill spec. Expected: skill@version"));
        }
        
        let (skill_name, version) = (skill_parts[0], skill_parts[1]);
        
        // Check cache first
        let cache_path = self.cache_dir
            .join(registry)
            .join(format!("{}@{}", skill_name, version));
        
        let component_path = cache_path.join("component.wasm");
        
        if component_path.exists() {
            println!("  ✓ Using cached component: {}", uri);
            return Ok(component_path);
        }

        
        println!("  ⬇ Downloading component: {}", uri);
        
        // Construct download URL (use http:// for localhost, https:// for production)
        let protocol = if registry.starts_with("localhost") { "http" } else { "https" };
        let base_url = format!("{}://{}/{}/{}", protocol, registry, skill_name, version);

        
        // Download component
        fs::create_dir_all(&cache_path).await?;
        
        let component_url = format!("{}/component.wasm", base_url);
        let manifest_url = format!("{}/manifest.toml", base_url);
        
        // Fetch manifest first for checksum
        let manifest_bytes = self.client
            .get(&manifest_url)
            .send()
            .await?
            .bytes()
            .await?;
        
        let manifest: toml::Value = toml::from_str(&String::from_utf8_lossy(&manifest_bytes))?;
        
        // Extract expected checksum
        let expected_checksum = manifest
            .get("checksums")
            .and_then(|c| c.get("component"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow!("Manifest missing component checksum"))?;
        
        // Download component
        let component_bytes = self.client
            .get(&component_url)
            .send()
            .await?
            .bytes()
            .await?;
        
        // Verify checksum
        if !self.verify_checksum(&component_bytes, expected_checksum)? {
            return Err(anyhow!("Checksum verification failed for {}", uri));
        }
        
        // Save to cache
        fs::write(&component_path, &component_bytes).await?;
        fs::write(cache_path.join("manifest.toml"), &manifest_bytes).await?;
        
        // Fetch and save interface.wit
        let wit_url = format!("{}/interface.wit", base_url);
        let wit_response = self.client.get(&wit_url).send().await;

        if let Ok(resp) = wit_response {
             if resp.status().is_success() {
                 let wit_bytes = resp.bytes().await?;
                 fs::write(cache_path.join("interface.wit"), &wit_bytes).await?;
             } else {
                 println!("  ⚠️  Warning: No interface.wit found for {}", uri);
             }
        } else {
             println!("  ⚠️  Warning: Failed to fetch interface.wit for {}", uri);
        }
        
        println!("  ✓ Downloaded and verified: {}", uri);
        
        Ok(component_path)
    }

    fn verify_checksum(&self, data: &[u8], expected: &str) -> Result<bool> {
        if !expected.starts_with("sha256:") {
            return Err(anyhow!("Only sha256 checksums are supported"));
        }
        
        let expected_hash = expected.strip_prefix("sha256:").unwrap();
        
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let computed_hash = format!("{:x}", result);
        
        Ok(computed_hash == expected_hash)
    }
}
