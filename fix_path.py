with open("src/main.rs", "r") as f:
    content = f.read()

import re

old_path = """path: "İndirmek için klasör seçin (Manuel)".to_string(), // They will need to pick a path to start!"""

new_path = """path: dirs::download_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("."))
    .join("ConnectSync")
    .join(name.replace("ConnectSync_", ""))
    .to_string_lossy()
    .to_string(),"""

content = content.replace(old_path, new_path)

with open("src/main.rs", "w") as f:
    f.write(content)
