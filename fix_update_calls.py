with open("src/main.rs", "r") as f:
    content = f.read()

import re

# In sync_loop_task, calls are update_status(&ui_weak, &app_state_loop, "Değişiklikler indiriliyor...", true);
content = content.replace(
    'update_status(&ui_weak, &app_state_loop, "Değişiklikler indiriliyor...", true);',
    'update_status(&ui_weak, &app_state_loop, &sync_code, "Değişiklikler indiriliyor...", true);'
)
content = content.replace(
    'update_status(&ui_weak, &app_state_loop, "Sync durduruldu.", false);',
    'update_status(&ui_weak, &app_state_loop, &sync_code, "Sync durduruldu.", false);'
)
content = content.replace(
    'update_status(&ui_weak, &app_state_loop, &format!("Pull hatası: {e}"), false);',
    'update_status(&ui_weak, &app_state_loop, &sync_code, &format!("Pull hatası: {e}"), false);'
)

# In run_push, run_pull we need to use sync_code instead of hardcoding.
content = content.replace(
    'update_status(ui_weak, app_state, "Eşitleniyor...", true);',
    'update_status(ui_weak, app_state, sync_code, "Eşitleniyor...", true);'
)
content = content.replace(
    'update_status(ui_weak, app_state, &msg, false);',
    'update_status(ui_weak, app_state, sync_code, &msg, false);'
)
content = content.replace(
    'update_error(ui_weak, app_state, &msg);',
    'update_error(ui_weak, app_state, sync_code, &msg);'
)

# the calls to run_push and run_pull
content = content.replace(
    'run_push(&engine, &ui_weak, &app_state_loop)',
    'run_push(&engine, &ui_weak, &app_state_loop, &sync_code)'
)
content = content.replace(
    'run_pull(&engine, &ui_weak, &app_state_loop)',
    'run_pull(&engine, &ui_weak, &app_state_loop, &sync_code)'
)

with open("src/main.rs", "w") as f:
    f.write(content)
