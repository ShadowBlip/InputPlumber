pub mod device_performance_menu;
pub mod device_test_menu;

use device_performance_menu::DevicePerformanceMenu;
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
    DevicePerformance(DevicePerformanceMenu),
}

impl Widget for &Menu {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        match self {
            Menu::DeviceTest(menu) => menu.render(area, buf),
            Menu::DevicePerformance(menu) => menu.render(area, buf),
        }
    }
}

impl MenuWidget for Menu {
    fn update(&mut self) -> Vec<InterfaceCommand> {
        match self {
            Menu::DeviceTest(menu) => menu.update(),
            Menu::DevicePerformance(menu) => menu.update(),
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Vec<InterfaceCommand> {
        match self {
            Menu::DeviceTest(menu) => menu.handle_key_event(key_event),
            Menu::DevicePerformance(menu) => menu.handle_key_event(key_event),
        }
    }
}

impl From<DeviceTestMenu> for Menu {
    fn from(value: DeviceTestMenu) -> Self {
        Self::DeviceTest(value)
    }
}

impl From<DevicePerformanceMenu> for Menu {
    fn from(value: DevicePerformanceMenu) -> Self {
        Self::DevicePerformance(value)
    }
}
