        textList = text.split("\n")
        if len(textList) >= 2:
            m = re.match(r"^\s*$", textList[-1])
            if m:
                current_indentation = m.group(0)
                previous_indentation = re.match(r"^\s*", textList[-2]).group(0)
                tab_size = settings.tab_size
                if len(current_indentation) >= settings.tab_size and \
                        current_indentation == previous_indentation:
                    event.current_buffer.delete_before_cursor(tab_size)

        event.current_buffer.insert_text(event.data)

    @handle('backspace', filter=insert_mode & default_focused & preceding_text(r"^\s+$"))
    def _(event):
        tab_size = settings.tab_size
        buf = event.current_buffer
        leading_spaces = len(buf.document.text_before_cursor)
        buf.delete_before_cursor(min(tab_size, leading_spaces))

    @handle('tab', filter=insert_mode & default_focused & preceding_text(r"^\s*$"))
    def _(event):
        tab_size = settings.tab_size
        event.current_buffer.insert_text(" " * tab_size)

    # bracketed paste
    @handle(Keys.BracketedPaste, filter=default_focused)
    def _(event):
        data = event.data

        data = data.replace('\r\n', '\n')
        data = data.replace('\r', '\n')

        should_eval = data and data[-1] == "\n" and \
            len(event.current_buffer.document.text_after_cursor) == 0
        # todo: allow partial prase complete
        if should_eval and parse_text_complete(data):
            data = data.rstrip("\n")
            event.current_buffer.insert_text(data)
            event.current_buffer.validate_and_handle()
        else:
            event.current_buffer.insert_text(data)

    return kb


# keybinds for both r mond and browse mode
def create_r_key_bindings(parse_text_complete):
    kb = create_prompt_key_bindings(parse_text_complete)
    handle = kb.add

    # r mode
    @handle(';', filter=insert_mode & default_focused & cursor_at_begin)
    def _(event):
        app = get_radian_app()
        app.session.activate_mode("shell")

    return kb


def create_shell_key_bindings():
    kb = KeyBindings()
    handle = kb.add

    # shell mode
    @handle(
        'backspace',
        filter=insert_mode & default_focused & cursor_at_begin,
        save_before=if_no_repeat)
    def _(event):
        app = get_radian_app()
        mode = app.session.mode_to_be_activated()
        app.session.activate_mode(mode)

    @handle('c-j', filter=insert_mode & default_focused)
    @handle('enter', filter=insert_mode & default_focused)
    def _(event):
        event.current_buffer.validate_and_handle()

    return kb


def create_key_bindings():
    kb = KeyBindings()
    handle = kb.add

    # emit completion
    @handle('c-j', filter=insert_mode & default_focused & completion_is_selected)
    @handle('enter', filter=insert_mode & default_focused & completion_is_selected)
    def _(event):
        event.current_buffer.complete_state = None

    # cancel completion
    @handle('c-c', filter=default_focused & has_completions)
    def _(event):
        event.current_buffer.cancel_completion()

    # new line
    @handle('escape', 'enter', filter=emacs_insert_mode)
    def _(event):
        if event.current_buffer.text:
            copy_margin = not in_paste_mode() and settings.auto_indentation
            event.current_buffer.newline(copy_margin=copy_margin)

    # Needed for to accept autosuggestions in vi insert mode
    @handle("c-e", filter=vi_focused_insert & ebivim)
    def _(event):
        b = event.current_buffer
        suggestion = b.suggestion
        if suggestion:
            b.insert_text(suggestion.text)
        else:
            nc.end_of_line(event)

    @handle("c-f", filter=vi_focused_insert & ebivim)
    def _(event):
        b = event.current_buffer
        suggestion = b.suggestion
        if suggestion:
            b.insert_text(suggestion.text)

