with open("src/main.rs", "r") as f:
    lines = f.readlines()

out = []
in_fetch = False
for line in lines:
    if "async fn fetch_cloud_folders" in line:
        in_fetch = True
    
    if in_fetch:
        if line.startswith("}") and "fetch_cloud_folders" not in "".join(out[-5:]): # heuristics
            pass # We'll just manually rewrite the end of the file.
