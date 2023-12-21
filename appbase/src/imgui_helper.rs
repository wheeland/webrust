use imgui::StyleColor;

pub fn staticwindow<'ui, F: FnOnce()> (
    ui: &'ui imgui::Ui,
    name: &str,
    pos: (f32, f32),
    size: (f32, f32),
    color: (f32, f32, f32, f32),
    f: F)
{
    let color = [color.0, color.1, color.2, color.3];
    let bg = ui.push_style_color(StyleColor::WindowBg, color);
    let border = ui.push_style_color(StyleColor::Border, color);
    ui.window(name)
        .position([pos.0, pos.1], imgui::Condition::Always)
        .size([size.0, size.1], imgui::Condition::Always)
        .title_bar(false)
        .movable(false)
        .resizable(false)
        .collapsible(false)
        .build(f);
    border.pop();
    bg.pop();
}
