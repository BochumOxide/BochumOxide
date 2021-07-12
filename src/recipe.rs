use crate::command::{create_command, Command, CommandCategory, CommandType};
use crate::utils::State;
use iced::button::{self};
use iced::Background;
use serde::{Deserialize, Serialize};

use iced_native::Button;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::gui::Message;
use iced::container;
use iced_graphics::Color;

use anyhow::Result;
use iced::{Align, Column, Container, Length, Row, Space, Text, TextInput};
use iced_native::text_input;
use iced_native::{pick_list, PickList};

static INGREDIENT_VIEW_ID_CTR: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Default, Debug)]
struct IngredientViewState {
    input: text_input::State,
    input2: text_input::State,
    output_choice: pick_list::State<String>,
    output_text: text_input::State,
    select_container: button::State,
    add: button::State,
    remove: button::State,
    move_up: button::State,
    move_down: button::State,
    output_changer: button::State,
}
#[derive(Serialize, Deserialize)]
pub struct IngredientView {
    pub title: String,
    description: String,
    #[serde(skip_serializing, default = "IngredientView::get_id")]
    pub id: usize,
    cmd_type: CommandType,
    pub output: String,
    pub input: String,
    #[serde(skip_serializing, skip_deserializing)]
    selected: bool,
    #[serde(skip_serializing, skip_deserializing)]
    show_output_text: bool,
    #[serde(skip_serializing, skip_deserializing)]
    state: IngredientViewState,
    has_input: bool,
    has_output: bool,
    pub category: CommandCategory,
}

impl Clone for IngredientView {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            description: self.description.clone(),
            id: IngredientView::get_id(),
            cmd_type: self.cmd_type,
            state: self.state.clone(),
            input: self.input.clone(),
            output: self.output.clone(),
            selected: self.selected,
            show_output_text: self.show_output_text,
            has_input: self.has_input,
            has_output: self.has_output,
            category: self.category,
        }
    }
}

pub struct IngredientStyle {
    selected: bool,
}

impl IngredientStyle {
    pub fn new() -> Self {
        IngredientStyle { selected: false }
    }

    pub fn selected(selected: bool) -> Self {
        IngredientStyle { selected }
    }
}

impl container::StyleSheet for IngredientStyle {
    fn style(&self) -> container::Style {
        let color = if self.selected {
            Color::from_rgb8(200, 200, 255)
        } else {
            Color::WHITE
        };

        container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        }
    }
}

impl button::StyleSheet for IngredientStyle {
    fn active(&self) -> button::Style {
        button::Style {
            ..Default::default()
        }
    }

    fn hovered(&self) -> button::Style {
        button::Style { ..self.active() }
    }
}

impl IngredientView {
    pub fn get_id() -> usize {
        INGREDIENT_VIEW_ID_CTR.fetch_add(1, Ordering::SeqCst)
    }
    pub fn new<T: Command + 'static>() -> Self {
        IngredientView {
            title: T::title(),
            description: T::description(),
            id: IngredientView::get_id(),
            cmd_type: T::cmd_type(), // save enum
            input: String::default(),
            output: String::default(),
            selected: false,
            show_output_text: false,
            state: IngredientViewState::default(),
            has_input: T::has_input(),
            has_output: T::has_output(),
            category: T::category(),
        }
    }

    pub fn run(&self, state: &mut State) -> Result<()> {
        let cmd = create_command(self.cmd_type, &self.input.as_bytes(), state);
        let res = cmd.execute(state)?;
        if !self.output.is_empty() && res.is_some() {
            state
                .registers
                .set(&self.output, res.expect("Clean this up later."));
        }
        Ok(())
    }

    pub fn draw_preview<'a>(&'a mut self) -> Container<'a, Message> {
        let title = Text::new(&self.title)
            .size(24)
            .color([0.2, 0.2, 0.2])
            .width(Length::FillPortion(25));

        let add_button = Button::new(&mut self.state.add, Text::new("+"))
            .width(Length::Shrink)
            .on_press(Message::AddIngredientPreview(self.id));

        let row = Row::new()
            .push(Space::with_width(Length::FillPortion(1)))
            .spacing(20)
            .push(title)
            .push(add_button);

        let mut column = Column::new()
            .align_items(Align::Start)
            .width(Length::Fill)
            .spacing(5)
            .push(row);

        let description = Text::new(&self.description)
            .color([0.2, 0.2, 0.2])
            .width(Length::FillPortion(9));
        if self.selected {
            let desc_row = Row::new()
                .push(Space::with_width(Length::FillPortion(1)))
                .push(description);
            column = column.push(desc_row);
        }

        let click_style: Box<dyn button::StyleSheet> = IngredientStyle::new().into();
        let clickable = Button::new(&mut self.state.select_container, column)
            .on_press(Message::SelectIngredientPreview(self.id))
            .width(Length::Fill)
            .style(click_style);

        let boxed_style: Box<dyn container::StyleSheet> =
            IngredientStyle::selected(self.selected).into();
        Container::new(clickable)
            .style(boxed_style)
            .width(Length::Fill)
    }

    pub fn draw_active<'a>(&'a mut self, registers: Vec<String>) -> Container<'a, Message> {
        let title = Text::new(&self.title).size(24).width(Length::Fill);
        let description = Text::new(&self.description);

        let remove_button = Button::new(&mut self.state.remove, Text::new("-"))
            .on_press(Message::RemoveIngredient(self.id));
        let move_up_button = Button::new(&mut self.state.move_up, Text::new("↑"))
            .on_press(Message::MoveIngredientUp(self.id));
        let move_down_button = Button::new(&mut self.state.move_down, Text::new("↓"))
            .on_press(Message::MoveIngredientDown(self.id));

        let title_row = Row::new()
            .spacing(5)
            .push(title)
            .push(move_up_button)
            .push(move_down_button)
            .push(remove_button)
            .width(Length::Shrink);

        let id = self.id;

        let mut row = Row::new();

        if self.has_input {
            let input = TextInput::new(
                &mut self.state.input,
                "Insert arguments",
                &self.input,
                move |msg| Message::IngredientDataChange(id, msg),
            );
            row = row.push(input);
        }

        if self.has_output {
            if self.show_output_text {
                let text_reg = TextInput::new(
                    &mut self.state.output_text,
                    "Name a register",
                    &self.output,
                    move |msg| Message::IngredientOutputChange(id, msg),
                )
                .on_submit(Message::CreateRegister(id));
                row = row.push(text_reg);
            } else {
                let picklist = PickList::new(
                    &mut self.state.output_choice,
                    registers,
                    Some(self.output.clone()),
                    move |msg| Message::IngredientOutputChange(id, msg),
                );
                row = row.push(picklist);
            }
            let output_changer = Button::new(&mut self.state.output_changer, Text::new("<>"))
                .on_press(Message::IngredientOutputChangeType(id));
            row = row.push(output_changer);
        }

        let column = Column::new()
            .align_items(Align::Start)
            .width(Length::Fill)
            .spacing(5)
            .push(title_row)
            .push(description)
            .push(row);

        let click_style: Box<dyn button::StyleSheet> = IngredientStyle::new().into();

        let clickable = Button::new(&mut self.state.select_container, column)
            .on_press(Message::SelectIngredient(self.id))
            .style(click_style);

        let boxed_style: Box<dyn container::StyleSheet> = IngredientStyle::new().into();
        Container::new(clickable)
            .style(boxed_style)
            .width(Length::Fill)
    }

    pub fn set_output(&mut self, output: String) {
        self.output = output;
    }
    pub fn set_input(&mut self, input: String) {
        self.input = input;
    }
    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }
    pub fn toggle_selected(&mut self) {
        self.selected = !self.selected;
    }
    pub fn toggle_output_type(&mut self) {
        self.show_output_text = !self.show_output_text;
    }
}

pub struct CategoryViewState {}
pub struct CategoryView {
    pub ingredients: Vec<IngredientView>,
    pub id: usize,
    open: bool,
    title: String,
    pub category: CommandCategory,
}

impl CategoryView {
    pub fn new(category: CommandCategory) -> Self {
        CategoryView {
            ingredients: Vec::new(),
            id: IngredientView::get_id(),
            open: false,
            title: category.title(),
            category,
        }
    }

    pub fn push(&mut self, ingredient: IngredientView) {
        self.ingredients.push(ingredient);
    }

    pub fn draw<'a>(&'a mut self) -> Container<'a, Message> {
        let title = Text::new(&self.title).size(30);

        let mut column = Column::new().push(title).padding(10);

        for ingredient in &mut self.ingredients {
            column = column.push(ingredient.draw_preview());
        }
        Container::new(column)
    }
}
