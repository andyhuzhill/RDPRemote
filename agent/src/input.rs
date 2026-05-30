//! Windows 输入注入模块

/// 输入事件类型
#[derive(Debug, Clone)]
pub enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseButton { button: MouseButton, pressed: bool },
    MouseWheel { delta: i32 },
    Keyboard { key: u16, pressed: bool },
}

/// 鼠标按钮类型
#[derive(Debug, Clone)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Windows 输入注入器
pub struct InputInjector;

impl InputInjector {
    pub fn new() -> Self {
        Self
    }

    /// 注入输入事件
    #[cfg(target_os = "windows")]
    pub fn inject(&self, event: InputEvent) -> anyhow::Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        use windows::Win32::Foundation::BOOL;

        match event {
            InputEvent::MouseMove { x, y } => {
                unsafe {
                    let result = SetCursorPos(x, y);
                    if result.as_bool() {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("SetCursorPos failed"))
                    }
                }
            }
            InputEvent::MouseButton { button, pressed } => {
                unsafe {
                    let event_type = match button {
                        MouseButton::Left if pressed => INPUT_TYPE::MOUSEEVENTF_LEFTDOWN,
                        MouseButton::Left => INPUT_TYPE::MOUSEEVENTF_LEFTUP,
                        MouseButton::Right if pressed => INPUT_TYPE::MOUSEEVENTF_RIGHTDOWN,
                        MouseButton::Right => INPUT_TYPE::MOUSEEVENTF_RIGHTUP,
                        MouseButton::Middle if pressed => INPUT_TYPE::MOUSEEVENTF_MIDDLEDOWN,
                        MouseButton::Middle => INPUT_TYPE::MOUSEEVENTF_MIDDLEUP,
                    };

                    let input = INPUT {
                        r#type: INPUT_TYPE::MOUSE,
                        Anonymous: INPUT_0 {
                            mi: MOUSEINPUT {
                                dx: 0,
                                dy: 0,
                                mouseData: 0,
                                dwFlags: event_type,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    };

                    let count = SendInput(&[input], std::mem::size_of::<INPUT>() as u32);
                    if count > 0 {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("SendInput failed for mouse button"))
                    }
                }
            }
            InputEvent::MouseWheel { delta } => {
                unsafe {
                    let input = INPUT {
                        r#type: INPUT_TYPE::MOUSE,
                        Anonymous: INPUT_0 {
                            mi: MOUSEINPUT {
                                dx: 0,
                                dy: 0,
                                mouseData: delta as u32,
                                dwFlags: INPUT_TYPE::MOUSEEVENTF_WHEEL,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    };

                    let count = SendInput(&[input], std::mem::size_of::<INPUT>() as u32);
                    if count > 0 {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("SendInput failed for mouse wheel"))
                    }
                }
            }
            InputEvent::Keyboard { key, pressed } => {
                unsafe {
                    let flags = if pressed {
                        INPUT_TYPE::KEYEVENTF_KEYDOWN
                    } else {
                        INPUT_TYPE::KEYEVENTF_KEYUP
                    };

                    let input = INPUT {
                        r#type: INPUT_TYPE::KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: VIRTUAL_KEY(key),
                                wScan: 0,
                                dwFlags: flags,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    };

                    let count = SendInput(&[input], std::mem::size_of::<INPUT>() as u32);
                    if count > 0 {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("SendInput failed for keyboard"))
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn inject(&self, _event: InputEvent) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("Input injection is only supported on Windows"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_event_creation() {
        let move_event = InputEvent::MouseMove { x: 100, y: 200 };
        assert!(matches!(move_event, InputEvent::MouseMove { x, y } if x == 100 && y == 200));

        let click_event = InputEvent::MouseButton { button: MouseButton::Left, pressed: true };
        assert!(matches!(click_event, InputEvent::MouseButton { button: MouseButton::Left, pressed: true }));

        let wheel_event = InputEvent::MouseWheel { delta: 120 };
        assert!(matches!(wheel_event, InputEvent::MouseWheel { delta: 120 }));

        let key_event = InputEvent::Keyboard { key: 0x41, pressed: true };
        assert!(matches!(key_event, InputEvent::Keyboard { key: 0x41, pressed: true }));
    }
}
