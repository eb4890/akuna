use anyhow::{Result, Context, anyhow};
use std::path::Path;
use std::fs;
use wit_parser::{Resolve, UnresolvedPackage};

pub struct WitLoader {
    pub resolve: Resolve,
    pub main_package: wit_parser::PackageId,
}

impl WitLoader {
    pub fn load(path: &Path) -> Result<Self> {
        let mut resolve = Resolve::new();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read WIT file: {:?}", path))?;
            
        let pkg = UnresolvedPackage::parse(path, &content)?;
        let main_package = resolve.push(pkg)?;
        
        Ok(Self {
            resolve,
            main_package,
        })
    }
    
    pub fn get_interface_exports(&self, interface_name: &str) -> Result<Vec<String>> {
        let pkg = &self.resolve.packages[self.main_package];
        
        // wit-parser 0.13: pkg.interfaces is IndexMap<String, InterfaceId>
        // Note: The key in interfaces map is the name.
        let interface_id = pkg.interfaces.get(interface_name)
            .or_else(|| {
                // Fallback: Try to match without package prefix (e.g. "calendar-api" vs "local:pkg/calendar-api")
                if let Some(pos) = interface_name.find('/') {
                    let short_name = &interface_name[pos+1..];
                    pkg.interfaces.get(short_name)
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                let keys: Vec<_> = pkg.interfaces.keys().collect();
                anyhow!("Interface '{}' not found in WIT package interfaces. Available: {:?}", interface_name, keys)
            })?;

        let iface = &self.resolve.interfaces[*interface_id];
        Ok(iface.functions.keys().cloned().collect())
    }
}
