pub mod device_test_menu;

use device_test_menu::DeviceTestMenu;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    prelude::{Buffer, Rect},
    widgets::Widget,
};

use super::InterfaceCommand;

/// A [MenuWidget] is a [Widget] that can also handle key input
pub trait MenuWidget {
    fn update(&mut self) -> Vec<InterfaceCommand> {
        vec![]
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Vec<InterfaceCommand> {
        match key_event.code {
            KeyCode::Char('q') => vec![InterfaceCommand::Quit],
            _ => vec![],
        }
    }
}

/// Enumeration of all available menus
#[derive(Debug)]
pub enum Menu {
    DeviceTest(DeviceTestMenu),
}

impl Widget for &Menu {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        match self {
            Menu::DeviceTest(device_test_menu) => device_test_menu.render(area, buf),
        }
    }
}

impl MenuWidget for Menu {
    fn update(&mut self) -> Vec<InterfaceCommand> {
        match self {
            Menu::DeviceTest(device_test_menu) => device_test_menu.update(),
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Vec<InterfaceCommand> {
        match self {
            Menu::DeviceTest(device_test_menu) => device_test_menu.handle_key_event(key_event),
        }
    }
}

impl From<DeviceTestMenu> for Menu {
    fn from(value: DeviceTestMenu) -> Self {
        Self::DeviceTest(value)
    }
}
