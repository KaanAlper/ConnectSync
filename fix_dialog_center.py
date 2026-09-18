with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# Fix SyncCodeDialog alignment
search_str = r"""                SyncCodeDialog \{\n                    confirmed\(code\) => \{"""
replace_str = """                HorizontalLayout {
                    alignment: center;
                    SyncCodeDialog {
                        confirmed(code) => {"""

text = re.sub(search_str, replace_str, text)

# We must close the HorizontalLayout as well!
# The SyncCodeDialog block is:
#                 SyncCodeDialog {
#                     confirmed(code) => {
#                         root.show_connect_dialog = false;
#                         root.connect_to_sync(code);
#                     }
#                     cancelled => {
#                         root.show_connect_dialog = false;
#                     }
#                 }

search_end_str = r"""                    cancelled => \{\n                        root\.show_connect_dialog = false;\n                    \}\n                \}"""
replace_end_str = """                    cancelled => {
                            root.show_connect_dialog = false;
                        }
                    }
                }"""

text = re.sub(search_end_str, replace_end_str, text)

with open("ui/main.slint", "w") as f:
    f.write(text)
