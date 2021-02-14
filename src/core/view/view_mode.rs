// Copyright (c) Carl-Erwin Griffith


/*

[user] -> (input event) ->  [view, doc] -> [modes]-> [function](<input_events_trigger>, doc, view) -> layout? -> [user]

type ModeFunction = fn(editor: &mut Editor, env: &mut EditorEnv, trigger: &Vec<input_event>, doc: &mut Doc, view: &mut View) -> | Status ok/err need layout ? |

let ptr : ModeFunction = cancel_input(editor: &mut Editor, env: &mut EditorEnv, trigger: &Vec<input_event>, doc: &mut Doc, view: &mut View)

will allow keyboard recording/keyboard macros
fallback if no match ?


InputMap<String, ModeFunction>

registerInputMap("text-mode", map)
map = ... InputMap["move_marks_backward"] = move_marks_backward; ...

core functions
push_input_map(map)
pop_input_map() // always keep default


ctrl+a, ctrl-v,


"text-mode"
{
    // movement
    pub fn move_marks_backward(&mut self);
    pub fn move_marks_forward(&mut self);
    pub fn move_marks_to_start_of_line(&mut self);
    pub fn move_marks_to_end_of_line(&mut self);
    pub fn move_marks_to_previous_line(&mut self);
    pub fn move_marks_to_next_line(&mut self);
    pub fn move_mark_to_screen_start(&mut self);
    pub fn move_mark_to_screen_end(&mut self);

    pub fn scroll_to_previous_screen(&mut self)
    pub fn scroll_up(&mut self, nb_lines: usize);
    pub fn move_mark_to_start_of_file(&mut self);
    pub fn center_arround_mark(&mut self);
    pub fn scroll_to_next_screen(&mut self);
    pub fn scroll_down_off_screen(&mut self, max_offset: u64, nb_lines: usize);
    pub fn scroll_down(&mut self, nb_lines: usize);

    // buffer change
    pub fn insert_codepoint_array(&mut self, array: &[char]);
    pub fn remove_until_end_of_word(&mut self);
    pub fn remove_previous_codepoint(&mut self);
    pub fn cut_to_end_of_line(&mut self) -> bool
    pub fn paste(&mut self);
    pub fn undo(&mut self);
    pub fn redo(&mut self);

    pub fn save_document(&mut self) -> bool;

    // selection
    pub fn button_press(&mut self, button: u32, x: i32, y: i32);
    pub fn button_release(&mut self, button: u32, _x: i32, _y: i32);

    // selections

*/
