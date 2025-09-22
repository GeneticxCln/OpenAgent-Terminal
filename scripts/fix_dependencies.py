#!/usr/bin/env python3
"""
Dependency Resolution Script for OpenAgent Terminal
Analyzes and fixes version conflicts in Cargo dependencies
"""

import subprocess
import json
import re
from pathlib import Path
from collections import defaultdict
from typing import Dict, Set, List, Tuple

def run_cargo_command(cmd: List[str]) -> str:
    """Run a cargo command and return output"""
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        return result.stdout
    except subprocess.CalledProcessError as e:
        print(f"Error running {' '.join(cmd)}: {e}")
        return ""

def extract_conflicts(duplicates_output: str) -> Dict[str, List[str]]:
    """Extract version conflicts from cargo tree --duplicates output"""
    conflicts = defaultdict(list)
    
    lines = duplicates_output.strip().split('\n')
    current_package = None
    
    for line in lines:
        # Match package versions like "base64 v0.21.7"
        package_match = re.match(r'^(\w+(?:-\w+)*)\s+v(.+)$', line)
        if package_match:
            current_package = package_match.group(1)
            version = package_match.group(2)
            conflicts[current_package].append(version)
    
    # Only return packages with multiple versions
    return {pkg: versions for pkg, versions in conflicts.items() if len(versions) > 1}

def analyze_dependencies():
    """Analyze current dependency conflicts"""
    print("🔍 Analyzing dependency conflicts...")
    
    duplicates_output = run_cargo_command(["cargo", "tree", "--duplicates"])
    conflicts = extract_conflicts(duplicates_output)
    
    print(f"\n📊 Found {len(conflicts)} packages with version conflicts:")
    
    critical_conflicts = {
        'base64': conflicts.get('base64', []),
        'rustix': conflicts.get('rustix', []),
        'sqlx-core': conflicts.get('sqlx-core', []),
        'bitflags': conflicts.get('bitflags', []),
        'tokio': conflicts.get('tokio', []),
        'futures-channel': conflicts.get('futures-channel', []),
        'hashbrown': conflicts.get('hashbrown', [])
    }
    
    print("\n🚨 Critical conflicts requiring resolution:")
    for pkg, versions in critical_conflicts.items():
        if versions:
            print(f"  {pkg}: {', '.join(versions)}")
    
    return conflicts, critical_conflicts

def find_cargo_tomls() -> List[Path]:
    """Find all Cargo.toml files in the workspace"""
    cargo_tomls = []
    
    # Workspace root
    workspace_toml = Path("Cargo.toml")
    if workspace_toml.exists():
        cargo_tomls.append(workspace_toml)
    
    # Find all crate Cargo.toml files
    for toml_file in Path(".").rglob("Cargo.toml"):
        if toml_file != workspace_toml and "target" not in str(toml_file):
            cargo_tomls.append(toml_file)
    
    return cargo_tomls

def read_cargo_toml(path: Path) -> str:
    """Read Cargo.toml content"""
    try:
        return path.read_text()
    except Exception as e:
        print(f"Error reading {path}: {e}")
        return ""

def write_cargo_toml(path: Path, content: str):
    """Write Cargo.toml content"""
    try:
        path.write_text(content)
        print(f"✅ Updated {path}")
    except Exception as e:
        print(f"Error writing {path}: {e}")

def fix_dependency_versions():
    """Fix dependency version conflicts"""
    print("\n🔧 Fixing dependency versions...")
    
    # Version resolutions for critical conflicts
    version_fixes = {
        'base64': '0.22.1',  # Use newer version
        'rustix': '1.1.2',   # Use newer version 
        'bitflags': '2.9.4', # Use newer version
        'tokio': '1.47.1',   # Already consistent
        'futures-channel': '0.3.31', # Already consistent
        'hashbrown': '0.16.0'  # Use newer version
    }
    
    cargo_tomls = find_cargo_tomls()
    
    for toml_path in cargo_tomls:
        content = read_cargo_toml(toml_path)
        if not content:
            continue
            
        original_content = content
        modified = False
        
        # Fix specific version patterns
        for package, target_version in version_fixes.items():
            # Match dependency lines like: package = "0.21.7"
            pattern = rf'({package}\s*=\s*["\'])([^"\']+)(["\'])'
            
            def replace_version(match):
                nonlocal modified
                if match.group(2) != target_version:
                    modified = True
                    print(f"  {toml_path}: {package} {match.group(2)} -> {target_version}")
                    return f"{match.group(1)}{target_version}{match.group(3)}"
                return match.group(0)
            
            content = re.sub(pattern, replace_version, content)
            
            # Also fix version patterns like: { version = "0.21.7", ... }
            pattern2 = rf'({package}\s*=\s*\{{[^}}]*version\s*=\s*["\'])([^"\']+)(["\'])'
            content = re.sub(pattern2, replace_version, content)
        
        if modified:
            write_cargo_toml(toml_path, content)

def update_workspace_dependencies():
    """Add workspace-level dependency version constraints"""
    print("\n📝 Updating workspace dependency constraints...")
    
    workspace_toml = Path("Cargo.toml")
    content = read_cargo_toml(workspace_toml)
    
    if not content:
        return
    
    # Check if [workspace.dependencies] section exists
    if "[workspace.dependencies]" not in content:
        # Add workspace dependencies section
        workspace_deps = '''

[workspace.dependencies]
# Unified dependency versions to resolve conflicts
base64 = "0.22.1"
rustix = "1.1.2" 
bitflags = "2.9.4"
hashbrown = "0.16.0"
tokio = { version = "1.47.1", features = ["full"] }
futures = "0.3.31"
serde = { version = "1.0.226", features = ["derive"] }
tracing = "0.1.41"
'''
        
        # Insert before the first [package] or [dependencies] section
        lines = content.split('\n')
        insert_index = len(lines)
        
        for i, line in enumerate(lines):
            if line.startswith('[') and not line.startswith('[workspace'):
                insert_index = i
                break
        
        lines.insert(insert_index, workspace_deps)
        content = '\n'.join(lines)
        
        write_cargo_toml(workspace_toml, content)

def validate_fixes():
    """Validate that fixes worked"""
    print("\n✅ Validating dependency fixes...")
    
    # Check if conflicts are resolved
    duplicates_output = run_cargo_command(["cargo", "tree", "--duplicates"])
    conflicts = extract_conflicts(duplicates_output)
    
    critical_packages = ['base64', 'rustix', 'sqlx-core', 'bitflags']
    remaining_conflicts = {pkg: versions for pkg, versions in conflicts.items() 
                          if pkg in critical_packages and len(versions) > 1}
    
    if remaining_conflicts:
        print("⚠️  Some conflicts remain:")
        for pkg, versions in remaining_conflicts.items():
            print(f"  {pkg}: {', '.join(versions)}")
        return False
    else:
        print("🎉 Critical dependency conflicts resolved!")
        return True

def main():
    """Main execution function"""
    print("🚀 OpenAgent Terminal Dependency Resolution")
    print("=" * 50)
    
    # Analyze current state
    conflicts, critical_conflicts = analyze_dependencies()
    
    if not any(critical_conflicts.values()):
        print("✅ No critical dependency conflicts found!")
        return
    
    # Apply fixes
    fix_dependency_versions()
    update_workspace_dependencies()
    
    # Validate
    print("\n🔄 Running cargo check to validate fixes...")
    result = subprocess.run(["cargo", "check"], capture_output=True, text=True)
    
    if result.returncode == 0:
        validate_fixes()
        print("\n✅ Dependency resolution completed successfully!")
    else:
        print(f"\n❌ Cargo check failed: {result.stderr}")
        print("Manual intervention may be required.")

if __name__ == "__main__":
    main()