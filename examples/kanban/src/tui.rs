use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use esruntime_sdk::prelude::Command;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::io;
use std::sync::Arc;
use std::{collections::HashMap, mem};
use umadb_client::SyncUmaDBClient;
use uuid::Uuid;

use crate::{
    commands::{
        change_task_status::{ChangeTaskStatus, ChangeTaskStatusInput},
        create_task::{CreateTask, CreateTaskInput},
        delete_task::{DeleteTask, DeleteTaskInput},
        rename_task::{RenameTask, RenameTaskInput},
    },
    projections::tasks::TasksProjection,
    types::TaskStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Column {
    Todo = 0,
    Doing = 1,
    Done = 2,
}

impl Column {
    fn status(&self) -> TaskStatus {
        match self {
            Column::Todo => TaskStatus::Todo,
            Column::Doing => TaskStatus::Doing,
            Column::Done => TaskStatus::Done,
        }
    }

    fn next(&self) -> Option<Self> {
        match self {
            Column::Todo => Some(Column::Doing),
            Column::Doing => Some(Column::Done),
            Column::Done => None,
        }
    }

    fn prev(&self) -> Option<Self> {
        match self {
            Column::Todo => None,
            Column::Doing => Some(Column::Todo),
            Column::Done => Some(Column::Doing),
        }
    }

    fn title(&self) -> &str {
        match self {
            Column::Todo => "📋 TODO",
            Column::Doing => "🔄 DOING",
            Column::Done => "✅ DONE",
        }
    }

    fn color(&self) -> Color {
        match self {
            Column::Todo => Color::Yellow,
            Column::Doing => Color::Cyan,
            Column::Done => Color::Green,
        }
    }
}

enum InputMode {
    Normal,
    Creating,
    Renaming,
}

struct KanbanState {
    selected_column: Column,
    selected_indices: HashMap<Column, usize>,
    input_mode: InputMode,
    new_task_name: String,
}

impl KanbanState {
    fn new() -> Self {
        Self {
            selected_column: Column::Todo,
            selected_indices: HashMap::new(),
            input_mode: InputMode::Normal,
            new_task_name: String::new(),
        }
    }

    fn selected_index(&self) -> usize {
        *self
            .selected_indices
            .get(&self.selected_column)
            .unwrap_or(&0)
    }

    fn set_selected_index(&mut self, index: usize) {
        self.selected_indices.insert(self.selected_column, index);
    }
}

pub struct KanbanApp {
    projection: Arc<TasksProjection>,
    client: Arc<SyncUmaDBClient>,
    state: KanbanState,
}

impl KanbanApp {
    pub fn new(projection: Arc<TasksProjection>, client: Arc<SyncUmaDBClient>) -> Self {
        Self {
            projection,
            client,
            state: KanbanState::new(),
        }
    }

    fn get_tasks_for_column(&self, column: Column) -> Vec<(Uuid, String)> {
        let tasks = self.projection.tasks.lock().unwrap();
        tasks
            .iter()
            .filter(|(_, (_, status))| *status == column.status())
            .map(|(id, (name, _))| (*id, name.clone()))
            .collect()
    }

    fn move_up(&mut self) {
        let tasks = self.get_tasks_for_column(self.state.selected_column);
        if tasks.is_empty() {
            return;
        }

        let current = self.state.selected_index();
        if current > 0 {
            self.state.set_selected_index(current - 1);
        }
    }

    fn move_down(&mut self) {
        let tasks = self.get_tasks_for_column(self.state.selected_column);
        if tasks.is_empty() {
            return;
        }

        let current = self.state.selected_index();
        if current < tasks.len().saturating_sub(1) {
            self.state.set_selected_index(current + 1);
        }
    }

    fn move_left(&mut self) {
        if let Some(prev) = self.state.selected_column.prev() {
            self.state.selected_column = prev;
            let tasks = self.get_tasks_for_column(self.state.selected_column);
            let current = self.state.selected_index();
            if current >= tasks.len() && !tasks.is_empty() {
                self.state.set_selected_index(tasks.len() - 1);
            }
        }
    }

    fn move_right(&mut self) {
        if let Some(next) = self.state.selected_column.next() {
            self.state.selected_column = next;
            let tasks = self.get_tasks_for_column(self.state.selected_column);
            let current = self.state.selected_index();
            if current >= tasks.len() && !tasks.is_empty() {
                self.state.set_selected_index(tasks.len() - 1);
            }
        }
    }

    fn move_task_left(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(prev_column) = self.state.selected_column.prev() {
            let tasks = self.get_tasks_for_column(self.state.selected_column);
            if let Some((task_id, _)) = tasks.get(self.state.selected_index()) {
                ChangeTaskStatus::execute_blocking(
                    self.client.as_ref(),
                    ChangeTaskStatusInput {
                        task_id: *task_id,
                        status: prev_column.status(),
                    },
                )?;
            }
        }
        Ok(())
    }

    fn move_task_right(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(next_column) = self.state.selected_column.next() {
            let tasks = self.get_tasks_for_column(self.state.selected_column);
            if let Some((task_id, _)) = tasks.get(self.state.selected_index()) {
                ChangeTaskStatus::execute_blocking(
                    self.client.as_ref(),
                    ChangeTaskStatusInput {
                        task_id: *task_id,
                        status: next_column.status(),
                    },
                )?;
            }
        }
        Ok(())
    }

    fn rename_task(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.state.new_task_name.is_empty() {
            let tasks = self.get_tasks_for_column(self.state.selected_column);
            if let Some((task_id, _)) = tasks.get(self.state.selected_index()) {
                RenameTask::execute_blocking(
                    self.client.as_ref(),
                    RenameTaskInput {
                        task_id: *task_id,
                        name: mem::take(&mut self.state.new_task_name),
                    },
                )?;
            }
        }
        self.state.input_mode = InputMode::Normal;
        Ok(())
    }

    fn delete_task(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let tasks = self.get_tasks_for_column(self.state.selected_column);
        if let Some((task_id, _)) = tasks.get(self.state.selected_index()) {
            DeleteTask::execute_blocking(
                self.client.as_ref(),
                DeleteTaskInput { task_id: *task_id },
            )?;

            // Adjust selection after deletion
            if self.state.selected_index() >= tasks.len().saturating_sub(1)
                && self.state.selected_index() > 0
            {
                self.state
                    .set_selected_index(self.state.selected_index() - 1);
            }
        }
        Ok(())
    }

    fn create_task(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.state.new_task_name.is_empty() {
            CreateTask::execute_blocking(
                self.client.as_ref(),
                CreateTaskInput {
                    task_id: Uuid::new_v4(),
                    name: mem::take(&mut self.state.new_task_name),
                    status: self.state.selected_column.status(),
                },
            )?;
        }
        self.state.input_mode = InputMode::Normal;
        Ok(())
    }

    fn render_column(&self, f: &mut Frame, area: Rect, column: Column) {
        let tasks = self.get_tasks_for_column(column);

        let is_selected = self.state.selected_column == column;
        let border_style = if is_selected {
            Style::default()
                .fg(column.color())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let items: Vec<ListItem> = tasks
            .iter()
            .enumerate()
            .map(|(i, (_, name))| {
                let is_task_selected = is_selected && i == self.state.selected_index();
                let style = if is_task_selected {
                    Style::default()
                        .bg(column.color())
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let prefix = if is_task_selected { "► " } else { "  " };
                ListItem::new(Line::from(format!("{}{}", prefix, name))).style(style)
            })
            .collect();

        let count_text = format!(" ({}) ", tasks.len());
        let block = Block::default()
            .title(Line::from(vec![
                Span::styled(
                    column.title(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(count_text),
            ]))
            .borders(Borders::ALL)
            .border_style(border_style);

        let list = List::new(items).block(block);
        f.render_widget(list, area);
    }

    fn render_help(&self, f: &mut Frame, area: Rect) {
        let help_text = match self.state.input_mode {
            InputMode::Normal => {
                "↑/↓: Navigate | ←/→: Change Column | Shift+←/→: Move Task | n: New Task | r: Rename | d: Delete | q: Quit"
            }
            InputMode::Creating => "Type task name | Enter: Create | Esc: Cancel",
            InputMode::Renaming => "Type task name | Enter: Rename | Esc: Cancel",
        };

        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        f.render_widget(help, area);
    }

    fn render_input_popup(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(format!(
                " New Task in {} ",
                self.state.selected_column.title()
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.state.selected_column.color()));

        let input = Paragraph::new(self.state.new_task_name.as_str())
            .block(block)
            .style(Style::default().fg(Color::White));

        f.render_widget(input, area);
    }

    fn ui(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(f.area());

        // Title
        let title = Paragraph::new("Kanban Board")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // Columns
        let column_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(chunks[1]);

        self.render_column(f, column_chunks[0], Column::Todo);
        self.render_column(f, column_chunks[1], Column::Doing);
        self.render_column(f, column_chunks[2], Column::Done);

        // Help
        self.render_help(f, chunks[2]);

        // Input popup
        if matches!(
            self.state.input_mode,
            InputMode::Creating | InputMode::Renaming
        ) {
            let popup_area = centered_rect(60, 20, f.area());
            f.render_widget(ratatui::widgets::Clear, popup_area);
            self.render_input_popup(f, popup_area);
        }
    }

    fn handle_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        match self.state.input_mode {
            InputMode::Normal => match key {
                KeyCode::Char('q') => return Ok(true),
                KeyCode::Up | KeyCode::Char('k') => self.move_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_down(),
                KeyCode::Left | KeyCode::Char('H') if modifiers.contains(KeyModifiers::SHIFT) => {
                    self.move_task_left()?;
                }
                KeyCode::Right | KeyCode::Char('L') if modifiers.contains(KeyModifiers::SHIFT) => {
                    self.move_task_right()?;
                }
                KeyCode::Left | KeyCode::Char('h') => self.move_left(),
                KeyCode::Right | KeyCode::Char('l') => self.move_right(),
                KeyCode::Char('n') => {
                    self.state.input_mode = InputMode::Creating;
                }
                KeyCode::Char('r') => {
                    let tasks = self.get_tasks_for_column(self.state.selected_column);
                    if let Some((_, name)) = tasks.get(self.state.selected_index()) {
                        self.state.new_task_name = name.clone();
                        self.state.input_mode = InputMode::Renaming;
                    }
                }
                KeyCode::Char('d') => {
                    self.delete_task()?;
                }
                _ => {}
            },
            InputMode::Creating => match key {
                KeyCode::Enter => {
                    self.create_task()?;
                }
                KeyCode::Esc => {
                    self.state.input_mode = InputMode::Normal;
                    self.state.new_task_name.clear();
                }
                KeyCode::Char(c) => {
                    self.state.new_task_name.push(c);
                }
                KeyCode::Backspace => {
                    self.state.new_task_name.pop();
                }
                _ => {}
            },
            InputMode::Renaming => match key {
                KeyCode::Enter => {
                    self.rename_task()?;
                }
                KeyCode::Esc => {
                    self.state.input_mode = InputMode::Normal;
                    self.state.new_task_name.clear();
                }
                KeyCode::Char(c) => {
                    self.state.new_task_name.push(c);
                }
                KeyCode::Backspace => {
                    self.state.new_task_name.pop();
                }
                _ => {}
            },
        }
        Ok(false)
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = self.run_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        if let Err(err) = res {
            eprintln!("Error: {}", err);
        }

        Ok(())
    }

    fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if event::poll(std::time::Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && self.handle_key(key.code, key.modifiers)?
            {
                return Ok(());
            }
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
