// Import the rust_minifb key enum and the X11 keysyms:
use minifb::Key;
use x11::keysym::*;

/// Converts a rust_minifb::Key into an X11 keysym (a u64).
///
/// This mapping uses the standard X keysyms:
/// - For letters and numbers the mapping is straightforward,
/// - For function keys and punctuation the corresponding keysym is returned.
///
/// Note: You may need to adjust or extend this mapping for your own needs.
pub fn map_minifb_key_to_x11_keysym(key: Key) -> u32 {
    match key {
        // Map numbers 0-9 (XK_0 = 0x0030, etc.)
        Key::Key0 => XK_0,
        Key::Key1 => XK_1,
        Key::Key2 => XK_2,
        Key::Key3 => XK_3,
        Key::Key4 => XK_4,
        Key::Key5 => XK_5,
        Key::Key6 => XK_6,
        Key::Key7 => XK_7,
        Key::Key8 => XK_8,
        Key::Key9 => XK_9,

        // Map letters (XK_A = 0x0041, etc.)
        Key::A => XK_A,
        Key::B => XK_B,
        Key::C => XK_C,
        Key::D => XK_D,
        Key::E => XK_E,
        Key::F => XK_F,
        Key::G => XK_G,
        Key::H => XK_H,
        Key::I => XK_I,
        Key::J => XK_J,
        Key::K => XK_K,
        Key::L => XK_L,
        Key::M => XK_M,
        Key::N => XK_N,
        Key::O => XK_O,
        Key::P => XK_P,
        Key::Q => XK_Q,
        Key::R => XK_R,
        Key::S => XK_S,
        Key::T => XK_T,
        Key::U => XK_U,
        Key::V => XK_V,
        Key::W => XK_W,
        Key::X => XK_X,
        Key::Y => XK_Y,
        Key::Z => XK_Z,

        // Function keys
        Key::F1  => XK_F1,
        Key::F2  => XK_F2,
        Key::F3  => XK_F3,
        Key::F4  => XK_F4,
        Key::F5  => XK_F5,
        Key::F6  => XK_F6,
        Key::F7  => XK_F7,
        Key::F8  => XK_F8,
        Key::F9  => XK_F9,
        Key::F10 => XK_F10,
        Key::F11 => XK_F11,
        Key::F12 => XK_F12,
        Key::F13 => XK_F13,
        Key::F14 => XK_F14,
        Key::F15 => XK_F15,

        // Arrow keys
        Key::Up    => XK_Up,
        Key::Down  => XK_Down,
        Key::Left  => XK_Left,
        Key::Right => XK_Right,

        // Punctuation and symbols
        Key::Apostrophe   => XK_apostrophe,  // typically the ' key
        Key::Backquote    => XK_grave,       // the ` key
        Key::Backslash    => XK_backslash,
        Key::Comma        => XK_comma,
        Key::Equal        => XK_equal,
        Key::LeftBracket  => XK_bracketleft,
        Key::Minus        => XK_minus,
        Key::Period       => XK_period,
        Key::RightBracket => XK_bracketright,
        Key::Semicolon    => XK_semicolon,
        Key::Slash        => XK_slash,

        // Control keys
        Key::Backspace => XK_BackSpace,
        Key::Delete    => XK_Delete,
        Key::End       => XK_End,
        Key::Enter     => XK_Return,
        Key::Escape    => XK_Escape,
        Key::Home      => XK_Home,
        Key::Insert    => XK_Insert,
        Key::Menu      => XK_Menu,
        Key::PageDown  => XK_Page_Down,
        Key::PageUp    => XK_Page_Up,
        Key::Pause     => XK_Pause,
        Key::Space     => XK_space,
        Key::Tab       => XK_Tab,

        // Lock and modifier keys
        Key::NumLock    => XK_Num_Lock,
        Key::CapsLock   => XK_Caps_Lock,
        Key::ScrollLock => XK_Scroll_Lock,
        Key::LeftShift  => XK_Shift_L,
        Key::RightShift => XK_Shift_R,
        Key::LeftCtrl   => XK_Control_L,
        Key::RightCtrl  => XK_Control_R,
        Key::LeftAlt    => XK_Alt_L,
        Key::RightAlt   => XK_Alt_R,
        Key::LeftSuper  => XK_Super_L,
        Key::RightSuper => XK_Super_R,

        // Numpad keys
        Key::NumPad0         => XK_KP_0,
        Key::NumPad1         => XK_KP_1,
        Key::NumPad2         => XK_KP_2,
        Key::NumPad3         => XK_KP_3,
        Key::NumPad4         => XK_KP_4,
        Key::NumPad5         => XK_KP_5,
        Key::NumPad6         => XK_KP_6,
        Key::NumPad7         => XK_KP_7,
        Key::NumPad8         => XK_KP_8,
        Key::NumPad9         => XK_KP_9,
        Key::NumPadDot       => XK_KP_Decimal,
        Key::NumPadSlash     => XK_KP_Divide,
        Key::NumPadAsterisk  => XK_KP_Multiply,
        Key::NumPadMinus     => XK_KP_Subtract,
        Key::NumPadPlus      => XK_KP_Add,
        Key::NumPadEnter     => XK_KP_Enter,

        // Fallback for unknown or unmapped keys
        Key::Unknown => 0,
        // In case new keys are added later, we default to 0.
        _ => 0,
    }
}
