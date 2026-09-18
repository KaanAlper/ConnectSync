with open("src/main.rs", "r") as f:
    lines = f.readlines()

out = []
for i, line in enumerate(lines):
    if line.strip() == "let ui_w3 = ui.as_weak();":
        if lines[i+1].strip().startswith("tokio::spawn(async move { fetch_cloud_folders"):
            continue # I will add it manually
        else:
            out.append(line)
    elif line.strip() == "let ui_w3 = ui_weak.clone();":
        continue
    elif line.strip().startswith("tokio::spawn(async move { fetch_cloud_folders"):
        continue
    else:
        out.append(line)

with open("src/main.rs", "w") as f:
    f.writelines(out)
