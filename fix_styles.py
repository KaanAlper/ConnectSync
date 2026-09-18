with open("ui/main.slint", "r") as f:
    text = f.read()

import re

# 1. Fix Logo Size & Text
old_logo_block = r"""                spacing: 5px;
                HorizontalLayout {
                    alignment: center;
                    Image {
                        source: @image-url\("\.\./assets/logo\.png"\);
                        width: 140px;
                        height: 70px;
                        image-fit: contain;
                    }
                }
                Text {
                    text: "ConnectSync";
                    font-size: 1.5rem;
                    font-weight: 800;
                    horizontal-alignment: center;
                    color: white;
                }
                Text {
                    text: "Zero-Knowledge P2P";
                    font-size: 0.8rem;
                    color: #a6adc8;
                    horizontal-alignment: center;
                }
                Rectangle { height: 15px; }"""

new_logo_block = """                spacing: 10px;
                HorizontalLayout {
                    alignment: center;
                    Image {
                        source: @image-url("../assets/logo.png");
                        width: 280px;
                        height: 133px;
                        image-fit: contain;
                    }
                }
                Text {
                    text: "ConnectSync";
                    font-size: 1.8rem;
                    font-weight: 700;
                    horizontal-alignment: center;
                    color: white;
                }
                Text {
                    text: "Serverless Folder Sync";
                    font-size: 1.1rem;
                    color: #a6adc8;
                    horizontal-alignment: center;
                }
                Rectangle { height: 2px; }"""

text = re.sub(old_logo_block, new_logo_block, text)

# 2. Fix the "3 koca mavi button"
# In the original UI, they were `ModernButton`. The user said "3 koca mavi button artık çirkin durdu".
# They liked the buttons but they were too "koca" (huge).
# I made them dark, and they said "renkler değişmiş düzelt mq" (colors changed, fix it).
# Let's make them light blue again, but make them nicely proportioned.
old_buttons_block = r"""                        Rectangle {
                            height: 40px;
                            border-radius: 8px;
                            border-width: 1px;
                            border-color: ta_btn1\.has-hover \? #89b4fa : #313244;
                            background: ta_btn1\.pressed \? #313244 : \(ta_btn1\.has-hover \? #1e1e2e : #11111b\);
                            Text { text: "Yeni Bir Sync Klasörü Oluştur"; color: #cdd6f4; font-size: 0\.95rem; font-weight: 600; horizontal-alignment: center; vertical-alignment: center; }
                            ta_btn1 := TouchArea { clicked => { root\.create_new_sync\(\) } }
                        }

                        Rectangle {
                            height: 40px;
                            border-radius: 8px;
                            border-width: 1px;
                            border-color: ta_btn2\.has-hover \? #89b4fa : #313244;
                            background: ta_btn2\.pressed \? #313244 : \(ta_btn2\.has-hover \? #1e1e2e : #11111b\);
                            Text { text: "Bir Sync Koduna Bağlan"; color: #cdd6f4; font-size: 0\.95rem; font-weight: 600; horizontal-alignment: center; vertical-alignment: center; }
                            ta_btn2 := TouchArea { clicked => { root\.show_connect_dialog = true; } }
                        }

                        Rectangle {
                            height: 40px;
                            border-radius: 8px;
                            background: ta_btn3\.pressed \? #74c7ec : \(ta_btn3\.has-hover \? #89dceb : #89b4fa\);
                            Text { text: "Senkronizasyonlarım"; color: #11111b; font-weight: 700; font-size: 0\.95rem; horizontal-alignment: center; vertical-alignment: center; }
                            ta_btn3 := TouchArea { clicked => { root\.show_my_syncs = true; } }
                        }"""

new_buttons_block = """                        ModernButton {
                            text: "Yeni Bir Sync Klasörü Oluştur";
                            clicked => { root.create_new_sync() }
                        }

                        ModernButton {
                            text: "Bir Sync Koduna Bağlan";
                            clicked => { root.show_connect_dialog = true; }
                        }

                        ModernButton {
                            text: "Senkronizasyonlarım";
                            clicked => { root.show_my_syncs = true; }
                        }"""

text = re.sub(old_buttons_block, new_buttons_block, text)

# The ModernButton is defined at the top. Let's make sure it's height: 40px instead of something crazy,
# actually it IS height: 40px. The reason it looked "koca" in the old screenshot was probably because `Ana Ekran`
# didn't have `width: 260px` or it was stretching to full width (350px)!
# Now `VerticalLayout` inside `Ana Ekran` has `width: 260px`. So `ModernButton` will be 260x40.
# That is a very nice, standard size. Not too huge.

with open("ui/main.slint", "w") as f:
    f.write(text)
