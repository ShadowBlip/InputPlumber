pub mod menu;
pub mod widgets;

use menu::{Menu, MenuWidget};
use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, Event, KeyEvent, KeyEventKind},
    layout::Rect,
    widgets::Widget,
    Frame,
};
use std::{error::Error, io, time::Duration};

/// InterfaceCommands are used to allow menus to communicate with the user interface
pub enum InterfaceCommand {
    /// Exit the interface
    Quit,
    /// Replace the current menu with the given menu
    #[allow(dead_code)]
    ReplaceMenu(Menu),
    /// Push the given menu to the menu stack
    #[allow(dead_code)]
    PushMenu(Menu),
    /// Pop the current menu from the menu stack
    #[allow(dead_code)]
    PopMenu,
}

/// The [Tui] is a text-based user interface for interacting with InputPlumber
#[derive(Debug, Default)]
pub struct TextUserInterface {
    exit: bool,
    current_menu: Option<Menu>,
    menu_stack: Vec<Menu>,
}

impl TextUserInterface {
    /// Create a new instance of the TUI
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the current menu with the given menu
    pub fn replace_menu(&mut self, menu: Menu) {
        self.current_menu = Some(menu);
    }

    /// Switch to the given menu
    pub fn push_menu(&mut self, menu: Menu) {
        let last_menu = self.current_menu.take();
        self.current_menu = Some(menu);
        if let Some(last_menu) = last_menu {
            self.menu_stack.push(last_menu);
        }
    }

    /// Pop the current menu from the stack
    pub fn pop_menu(&mut self) {
        self.current_menu = self.menu_stack.pop();
        if self.current_menu.is_none() {
            self.exit = true;
        }
    }

    /// Run the text interface
    pub fn run(&mut self, menu: Menu) -> Result<(), Box<dyn Error>> {
        self.current_menu = Some(menu);
        let mut terminal = ratatui::init();
        while !self.exit {
            self.handle_events()?;
            self.update();
            terminal.draw(|frame| self.draw(frame))?;
        }

        ratatui::restore();
        Ok(())
    }

    /// Update all menus
    fn update(&mut self) {
        let Some(menu) = self.current_menu.as_mut() else {
            return;
        };
        let mut commands = menu.update();
        for menu in self.menu_stack.iter_mut() {
            commands.extend(menu.update());
        }
        self.handle_commands(commands);
    }

    /// Draw a frame to the terminal
    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    /// Handle menu commands
    fn handle_commands(&mut self, commands: Vec<InterfaceCommand>) {
        for cmd in commands {
            match cmd {
                InterfaceCommand::Quit => self.exit = true,
                InterfaceCommand::ReplaceMenu(menu) => self.replace_menu(menu),
                InterfaceCommand::PushMenu(menu) => self.push_menu(menu),
                InterfaceCommand::PopMenu => self.pop_menu(),
            }
        }
    }

    /// Updates the application's state based on user input
    fn handle_events(&mut self) -> io::Result<()> {
        // Poll for events
        if !event::poll(Duration::from_millis(50))? {
            return Ok(());
        }
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    /// Handles key input to the terminal
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        let Some(menu) = self.current_menu.as_mut() else {
            return;
        };
        let commands = menu.handle_key_event(key_event);
        self.handle_commands(commands);
    }
}

impl Widget for &TextUserInterface {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Some(menu) = self.current_menu.as_ref() else {
            return;
        };
        for menu in self.menu_stack.iter() {
            menu.render(area, buf);
        }
        menu.render(area, buf);
    }
}
