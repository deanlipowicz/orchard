        if os.path.exists(xdg_profile):
            source_file(xdg_profile)

        global_profile = make_path("~", ".radian_profile")
        local_profile = make_path(".radian_profile")

        if os.path.exists(global_profile):
            source_file(global_profile)
        elif sys.platform.startswith("win"):
            # for backward compatibility
            global_profile = user_path(".radian_profile")
            if os.path.exists(global_profile):
                source_file(global_profile)

        if os.path.exists(local_profile) and local_profile != global_profile:
            source_file(local_profile)


def load_custom_key_bindings(*args):
    esc_keymap = roption("radian.escape_key_map", [])
    for m in esc_keymap:
        map_key(("escape", m["key"]), m["value"], mode=m["mode"] if "mode" in m else "r")

    keymap = roption("radian.ctrl_key_map", [])
    for m in keymap:
        if m["key"] in "mihdc":
            print("WARNING: Cannot remap c-" + m["key"] + ". Please remove this mapping from radian.ctrl_key_map in your radian profile")
        else:
            map_key(("c-" + m["key"],), m["value"], mode=m["mode"] if "mode" in m else "r")



def register_cleanup(cleanup):
    rcall(("base", "reg.finalizer"),
          rcall(("base", "getOption"), "rchitect.py_tools"),
          cleanup,
          onexit=True)


def set_utf8():
    if sys.platform.startswith("win"):
        ucrt = rcopy(
            reval('compareVersion(paste0(R.version$major, ".", R.version$minor), "4.2.0") >= 0'))
        if ucrt:
            if not os.environ.get("LANG", ""):
                os.environ["LANG"] = "en_US.UTF-8"
            setoption("encoding", "UTF-8")


def run_on_load_hooks():
    hooks = roption("radian.on_load_hooks", [])
    for hook in hooks:
        hook()

