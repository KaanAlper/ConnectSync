with open("src/main.rs", "r") as f:
    content = f.read()

import re

old_clip = re.search(r'    ui\.on_copy_to_clipboard\(move \|.*?\}\);\n    \}\);', content, re.DOTALL).group(0)
# Wait, the match above might match too much. Let's just match the exact lines.
