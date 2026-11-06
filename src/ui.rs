use super::app::{App, CurrentScreen, Data, ITEM_HEIGHT, InputMode};
use ratatui::{
	Frame,
	layout::{Constraint, Direction, Layout, Margin, Rect},
	style::{Color, Modifier, Style, Stylize, palette::tailwind},
	text::Text,
	widgets::{
		Block, BorderType, Borders, Cell, Clear, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarOrientation,
		ScrollbarState, Table, Wrap,
	},
};
use unicode_width::UnicodeWidthStr;

const BAR_SYMBOL: &str = " █ ";
const SCROLLBAR_BEGIN_SYMBOL: &str = "▲";
const SCROLLBAR_END_SYMBOL: &str = "▼";
const INFO_TEXT: [&str; 1] =
	["(Esc) quit | (n) new password | (Enter) send password | (d) delete password | (m) modify password | (?) help"];
const HELP_TEXT: [&str; 15] = [
	"--- Global --- ",
	"(Esc) - quit",
	"(n) - new password",
	"(d) - delete password",
	"(m) - modify password",
	"(Enter) - send password",
	"(c) - copy column",
	"(p) - copy password",
	"(P) - copy row",
	"\n",
	"--- Navigation ---",
	"(↑, k) - move up ",
	"(↓, j) - move down",
	"(←, h) - move left ",
	"(→, l) - move right",
];

#[derive(Default)]
pub struct TableColors {
	pub alt_row_color: Color,
	pub buffer_bg: Color,
	pub footer_border_color: Color,
	pub header_bg: Color,
	pub header_fg: Color,
	pub normal_row_color: Color,
	pub row_fg: Color,
	pub selected_cell_style_fg: Color,
	pub selected_column_style_fg: Color,
	pub selected_row_style_fg: Color,
}

impl TableColors {
	pub const fn new(color: &tailwind::Palette) -> Self {
		Self {
			buffer_bg: Color::Reset,
			header_bg: Color::Reset,
			header_fg: tailwind::SLATE.c200,
			row_fg: tailwind::SLATE.c50,
			selected_row_style_fg: color.c400,
			selected_column_style_fg: color.c400,
			selected_cell_style_fg: color.c600,
			normal_row_color: Color::Reset,
			alt_row_color: Color::Reset,
			footer_border_color: color.c50,
		}
	}
}

pub fn ui(frame: &mut Frame, app: &mut App) {
	let vertical = Layout::vertical([Constraint::Max(3), Constraint::Min(5), Constraint::Max(3)]);
	let rects = vertical.split(frame.area());

	render_header(app, frame, rects[0]);
	render_table(app, frame, rects[1]);
	render_scrollbar(
		frame,
		rects[1].inner(Margin {
			vertical: 1,
			horizontal: 1,
		}),
		&mut app.table.scroll_state,
	);

	render_footer(app, frame, rects[2]);

	match app.current_screen {
		CurrentScreen::Popup(..) => render_popup(app, frame),
		CurrentScreen::Help => render_help(frame),
		CurrentScreen::Error => render_error(app, frame),
		CurrentScreen::Main => {}
	}
}

fn render_help(frame: &mut Frame) {
	let area = centered_rect(40, 40, frame.area());
	let block = Block::default().title("Help").bg(Color::Reset).borders(Borders::ALL);
	let paragraph = Paragraph::new(Text::from_iter(HELP_TEXT))
		.block(block.clone())
		.alignment(ratatui::layout::Alignment::Center);

	frame.render_widget(Clear, area);
	frame.render_widget(paragraph, area);
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
		.split(popup_layout[1])[1] // Return the middle chunk
}

fn render_header(app: &App, frame: &mut Frame, area: Rect) {
	let block = Block::default()
		.borders(Borders::TOP | Borders::BOTTOM)
		.bg(app.table.colors.header_bg);
	let header_style = Style::default().fg(app.table.colors.header_fg);
	let chunks = Layout::default()
		.direction(Direction::Horizontal)
		.constraints([
			Constraint::Ratio(1, 3),
			Constraint::Ratio(1, 3),
			Constraint::Ratio(1, 3),
		])
		.split(block.inner(area));

	frame.render_widget(block, area);
	frame.render_widget(Paragraph::new("  Service").style(header_style), chunks[0]);
	frame.render_widget(Paragraph::new("  Login").style(header_style), chunks[1]);
	frame.render_widget(Paragraph::new("  Password").style(header_style), chunks[2]);
}

fn render_table(app: &mut App, frame: &mut Frame, area: Rect) {
	let selected_row_style = Style::default()
		.add_modifier(Modifier::REVERSED)
		.fg(app.table.colors.selected_row_style_fg);
	let selected_col_style = Style::default().fg(app.table.colors.selected_column_style_fg);
	let selected_cell_style = Style::default()
		.add_modifier(Modifier::REVERSED)
		.fg(app.table.colors.selected_cell_style_fg);

	let rows = app.items.iter().enumerate().map(|(i, data)| {
		let color = match i % 2 {
			0 => app.table.colors.normal_row_color,
			_ => app.table.colors.alt_row_color,
		};
		data.ref_array()
			.into_iter()
			.map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
			.collect::<Row>()
			.style(Style::new().fg(app.table.colors.row_fg).bg(color))
			.height(ITEM_HEIGHT)
	});

	let longest_item_lens = constraint_len_calculator(&app.items);
	let table = Table::new(
		rows,
		[
			Constraint::Min(longest_item_lens.0 + 1),
			Constraint::Min(longest_item_lens.1 + 1),
			Constraint::Min(longest_item_lens.2),
		],
	)
	.row_highlight_style(selected_row_style)
	.column_highlight_style(selected_col_style)
	.cell_highlight_style(selected_cell_style)
	.highlight_symbol(Text::from(vec!["".into(), BAR_SYMBOL.into(), "".into()]))
	.bg(app.table.colors.buffer_bg)
	.highlight_spacing(HighlightSpacing::Always);

	frame.render_stateful_widget(table, area, &mut app.table.table_state);
}

fn render_scrollbar(frame: &mut Frame, inner: Rect, scroll_state: &mut ScrollbarState) {
	frame.render_stateful_widget(
		Scrollbar::default()
			.orientation(ScrollbarOrientation::VerticalRight)
			.begin_symbol(Some(SCROLLBAR_BEGIN_SYMBOL))
			.end_symbol(Some(SCROLLBAR_END_SYMBOL)),
		inner,
		scroll_state,
	);
}

pub fn render_popup(app: &App, frame: &mut Frame) {
	let popup_block = Block::default()
		.title("Form")
		.borders(Borders::ALL)
		.style(Style::default())
		.bg(Color::Reset);
	let area = centered_rect(60, 37, frame.area());

	frame.render_widget(Clear, area);
	frame.render_widget(popup_block, area);

	let popup_chunks = Layout::default()
		.direction(Direction::Vertical)
		.margin(1)
		.constraints([Constraint::Fill(1), Constraint::Fill(1), Constraint::Fill(1)])
		.split(area);

	let mut service_block = Block::default().title("Service").borders(Borders::ALL);
	let mut login_block = Block::default().title("Login or Email").borders(Borders::ALL);
	let mut password_block = Block::default().title("Password").borders(Borders::ALL);

	let active_style = Style::default().fg(Color::Blue);

	match app.input_mode {
		InputMode::Service => service_block = service_block.style(active_style),
		InputMode::Login => login_block = login_block.style(active_style),
		InputMode::Password => password_block = password_block.style(active_style),
	}

	let service_text = Paragraph::new(app.input.service())
		.fg(Color::White)
		.block(service_block);
	frame.render_widget(service_text, popup_chunks[0]);

	let login_text = Paragraph::new(app.input.login()).fg(Color::White).block(login_block);
	frame.render_widget(login_text, popup_chunks[1]);

	let password = Paragraph::new(app.input.password())
		.fg(Color::White)
		.block(password_block);
	frame.render_widget(password, popup_chunks[2]);

	let (chunk, text) = match app.input_mode {
		InputMode::Service => (popup_chunks[0], &app.input.service),
		InputMode::Login => (popup_chunks[1], &app.input.login),
		InputMode::Password => (popup_chunks[2], &app.input.password),
	};

	let width = chunk.width.max(3) - 3;
	let scroll = text.visual_scroll(width as usize);
	let x = text.visual_cursor().max(scroll) - scroll + 1;
	frame.set_cursor_position((chunk.x + u16::try_from(x).unwrap_or(0), chunk.y + 1));
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
	let info_footer = Paragraph::new(Text::from_iter(INFO_TEXT))
		.style(Style::new().fg(app.table.colors.row_fg).bg(app.table.colors.buffer_bg))
		.centered()
		.block(
			Block::bordered()
				.border_type(BorderType::Double)
				.border_style(Style::new().fg(app.table.colors.footer_border_color)),
		);
	frame.render_widget(info_footer, area);
}

fn render_error(app: &mut App, frame: &mut Frame) {
	let error_block = Block::default()
		.title("Error")
		.borders(Borders::ALL)
		.style(Style::default())
		.fg(Color::Red)
		.bg(Color::Reset);
	let area = centered_rect(60, 37, frame.area());

	frame.render_widget(Clear, area);
	frame.render_widget(
		Paragraph::new(app.error_message.as_str())
			.block(error_block)
			.wrap(Wrap { trim: false })
			.scroll((app.error_scroll, 0)),
		area,
	);

	let scrollbar_area = area.inner(Margin {
		vertical: 1,
		horizontal: 0,
	});
	app.error_scrollbar = app
		.error_scrollbar
		.position(usize::from(app.error_scroll))
		.viewport_content_length(usize::from(scrollbar_area.height));

	render_scrollbar(frame, scrollbar_area, &mut app.error_scrollbar);
}

fn constraint_len_calculator<T: Data>(items: &[T]) -> (u16, u16, u16) {
	let service_len = items
		.iter()
		.map(Data::service)
		.map(UnicodeWidthStr::width)
		.max()
		.and_then(|len| u16::try_from(len).ok())
		.unwrap_or_default();

	let login_len = items
		.iter()
		.map(Data::login)
		.map(UnicodeWidthStr::width)
		.max()
		.and_then(|len| u16::try_from(len).ok())
		.unwrap_or_default();

	let password_len = items
		.iter()
		.map(Data::password)
		.map(UnicodeWidthStr::width)
		.max()
		.and_then(|len| u16::try_from(len).ok())
		.unwrap_or_default();

	(service_len, login_len, password_len)
}

