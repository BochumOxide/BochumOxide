use crate::command::{CommandCategory, CustomIngredient};
use std::fs;
use std::fs::File;

use log::*;

use crate::command::available_categories;
use crate::recipe::{CategoryView, IngredientView};
use crate::utils::State;
use crate::utils::Target;
use iced::{
    button, executor, pick_list, scrollable, text_input, Align, Application, Button, Checkbox,
    Clipboard, Column, Command, Container, Element, Length, PickList, Row, Rule, Scrollable, Text,
    TextInput,
};

pub enum Scene {
    ChooseProgram,
    Recipe,
}

#[derive(Default)]
pub struct GuiState {
    program_name: text_input::State,
    start_button: button::State,
    run_all: button::State,
    recipe_scrollable: scrollable::State,
    ingredient_scrollable: scrollable::State,
    debug_scrollable: scrollable::State,
    program_output_scrollable: scrollable::State,
    load_recipe_file: pick_list::State<String>,
    save_recipe_file: text_input::State,
    load_recipe: button::State,
    save_recipe: button::State,
    save_ingredient: button::State,
}
pub struct App {
    current_scene: Scene,
    enabled: bool,
    should_exit: bool,
    state: Option<State>,
    debug_output: String,
    program_output: String,
    category_list: Vec<CategoryView>,
    recipe: Vec<IngredientView>,
    save_recipe_name: String,
    load_recipe_name: String,
    program_name: String,
    is_network: bool,
    gui_state: GuiState,
}

#[derive(Debug, Clone)]
pub enum Message {
    IngredientOutputChange(usize, String),
    IngredientDataChange(usize, String),
    IngredientOutputChangeType(usize),
    SelectIngredient(usize),
    SelectIngredientPreview(usize),
    AddIngredientPreview(usize),
    RemoveIngredient(usize),
    MoveIngredientUp(usize),
    MoveIngredientDown(usize),
    SaveRecipe,
    LoadRecipe,
    SaveIngredient,
    ProgramNameChanged(String),
    CreateRegister(usize),
    IsNetworkChanged(bool),
    StartProgram,
    RunAll,
    SaveRecipeChanged(String),
    LoadRecipeChanged(String),
}

impl App {
    fn load_log(&mut self) {
        self.debug_output = std::fs::read_to_string("log.log").unwrap();
    }

    fn load_custom_ingredients(&mut self) {
        if let Err(e) = fs::create_dir_all("ingredients/") {
            panic!("Could not create ingredients directory.");
        }

        let custom_ingredients: Vec<String> = fs::read_dir("ingredients/")
            .unwrap()
            .filter_map(|maybe_dir_entry| {
                let path_buf = maybe_dir_entry.ok()?.path();
                let file_name = path_buf.file_name()?;
                let string = file_name.to_str()?;
                Some(string.to_string())
            })
            .collect();

        let iviews: Vec<_> = custom_ingredients
            .into_iter()
            .map(|path| {
                let mut iview = IngredientView::new::<CustomIngredient>();
                iview.input = path.clone();
                iview.title = path;
                iview
            })
            .collect();

        for category in &mut self.category_list {
            if category.category == CommandCategory::Custom {
                category.ingredients = iviews;
                break;
            }
        }
    }

    fn ingredient_list(&mut self) -> impl Iterator<Item = &mut IngredientView> {
        self.category_list
            .iter_mut()
            .map(|cat| cat.ingredients.iter_mut())
            .flatten()
    }

    fn view_choose_program(&mut self) -> Element<Message> {
        let program_name_input = TextInput::new(
            &mut self.gui_state.program_name,
            "Name a program",
            &self.program_name,
            |msg| Message::ProgramNameChanged(msg),
        )
        .width(Length::Units(300))
        .on_submit(Message::StartProgram);

        let start_button =
            Button::new(&mut self.gui_state.start_button, Text::new("Start working"))
                .on_press(Message::StartProgram);

        let is_network_checkbox =
            Checkbox::new(self.is_network, "Network", Message::IsNetworkChanged);

        let row = Row::new()
            .push(program_name_input)
            .push(is_network_checkbox)
            .align_items(Align::Center)
            .spacing(10);

        let col = Column::new()
            .push(row)
            .push(start_button)
            .align_items(Align::Center)
            .spacing(4);

        Container::new(col)
            .center_x()
            .center_y()
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }

    fn view_recipe(&mut self) -> Element<Message> {
        let recipe_header = Container::new(Text::new("Recipe").size(50))
            .width(Length::FillPortion(1))
            .padding(20);

        let run_button = Button::new(&mut self.gui_state.run_all, Text::new("Run all"))
            .on_press(Message::RunAll);

        let save_recipe_button =
            Button::new(&mut self.gui_state.save_recipe, Text::new("Save as recipe"))
                .on_press(Message::SaveRecipe);

        let save_ingredient_button = Button::new(
            &mut self.gui_state.save_ingredient,
            Text::new("Save as ingredient"),
        )
        .on_press(Message::SaveIngredient);
        let save_ingredient_container = Container::new(save_ingredient_button)
            .align_x(Align::End)
            .width(Length::Fill);

        let load_recipe_button =
            Button::new(&mut self.gui_state.load_recipe, Text::new("Load recipe"))
                .on_press(Message::LoadRecipe);

        let ingredients_header = Container::new(Text::new("Ingredients").size(50))
            .width(Length::FillPortion(1))
            .padding(20);

        let mut ingredient_scroller = Scrollable::new(&mut self.gui_state.ingredient_scrollable)
            .spacing(2)
            .width(Length::Fill)
            .height(Length::Fill);

        for category in &mut self.category_list {
            ingredient_scroller = ingredient_scroller.push(category.draw());
        }

        let mut recipe_scroller = Scrollable::new(&mut self.gui_state.recipe_scrollable)
            .spacing(2)
            .width(Length::Fill)
            .height(Length::Fill);

        let registers = self.state.as_ref().unwrap().registers.available_registers();

        for ingredient in &mut self.recipe {
            recipe_scroller = recipe_scroller.push(ingredient.draw_active(registers.clone()));
        }

        // save
        let save_recipe_input = TextInput::new(
            &mut self.gui_state.save_recipe_file,
            "Recipe/Ingredient Name",
            &self.save_recipe_name,
            move |msg| Message::SaveRecipeChanged(msg),
        );
        let save_recipe_row = Row::new()
            .spacing(20)
            .push(save_recipe_input)
            .push(save_recipe_button);

        // load
        fs::create_dir_all("recipes/").expect("Could not create recipes directory");
        let saved_recipes: Vec<String> = fs::read_dir("recipes/")
            .unwrap()
            .filter_map(|maybe_dir_entry| {
                let path_buf = maybe_dir_entry.ok()?.path();
                let file_name = path_buf.file_name()?;
                let string = file_name.to_str()?;
                Some(string.to_string())
            })
            .collect();

        let picklist = PickList::new(
            &mut self.gui_state.load_recipe_file,
            saved_recipes,
            Some(self.load_recipe_name.clone()),
            move |msg| Message::LoadRecipeChanged(msg),
        );

        let load_recipe_row = Row::new()
            .spacing(20)
            .push(picklist)
            .push(load_recipe_button);

        let recipes = Column::new()
            .align_items(Align::Start)
            .width(Length::FillPortion(3))
            .spacing(10)
            .push(recipe_header)
            .push(Rule::horizontal(0))
            .push(recipe_scroller)
            .push(Rule::horizontal(0))
            .push(save_recipe_row)
            .push(save_ingredient_container)
            .push(load_recipe_row)
            .push(run_button);

        let ingredients = Column::new()
            .align_items(Align::Start)
            .width(Length::FillPortion(2))
            .spacing(10)
            .push(ingredients_header)
            .push(Rule::horizontal(0))
            .push(ingredient_scroller);

        let output_content = Text::new(&self.debug_output).size(18);
        let output_scroller = Scrollable::new(&mut self.gui_state.debug_scrollable)
            .spacing(2)
            .width(Length::Fill)
            .height(Length::FillPortion(4))
            .push(output_content);

        let program_output = Text::new(&self.state.as_ref().unwrap().output).size(18);
        let program_output_scroller =
            Scrollable::new(&mut self.gui_state.program_output_scrollable)
                .spacing(2)
                .width(Length::Fill)
                .height(Length::FillPortion(4))
                .push(program_output);

        let output = Column::new()
            .align_items(Align::Start)
            .width(Length::FillPortion(3))
            .push(Text::new("Program Output").size(50))
            .push(Rule::horizontal(0))
            .push(program_output_scroller)
            .push(Text::new("Debug Output").size(50))
            .push(Rule::horizontal(0))
            .push(output_scroller);

        let content = Row::new()
            .align_items(Align::Center)
            .spacing(20)
            .push(ingredients)
            .push(Rule::vertical(0))
            .push(recipes)
            .push(Rule::vertical(0))
            .push(output);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: ()) -> (App, Command<Message>) {
        let mut app = App {
            current_scene: Scene::ChooseProgram,
            state: None,
            enabled: false,
            should_exit: false,
            debug_output: String::new(),
            program_output: String::new(),
            category_list: available_categories(),
            recipe: Vec::new(),
            program_name: String::default(),
            is_network: false,
            save_recipe_name: String::default(),
            load_recipe_name: String::default(),
            gui_state: Default::default(),
        };
        app.gui_state.program_name.focus();
        app.load_custom_ingredients();
        (app, Command::none())
    }

    fn title(&self) -> String {
        String::from("BochumOxide")
    }
    fn update(&mut self, message: Message, _clipboard: &mut Clipboard) -> Command<Message> {
        match message {
            Message::AddIngredientPreview(id) => {
                let ingredient = self.ingredient_list().find(|i| i.id == id).cloned();
                if let Some(ingredient) = ingredient {
                    self.recipe.push(ingredient);
                }
            }
            Message::StartProgram => {
                self.current_scene = Scene::Recipe;
                let mut state = State::new(
                    if self.is_network {
                        Target::Network
                    } else {
                        Target::Local
                    },
                    &self.program_name,
                    &[],
                )
                .expect("Failed to spawn program");
                state
                    .registers
                    .set("program", self.program_name.as_bytes().to_vec());
                self.state = Some(state);
            }
            Message::ProgramNameChanged(name) => {
                self.program_name = name;
            }
            Message::IsNetworkChanged(enabled) => {
                self.is_network = enabled;
            }
            Message::MoveIngredientUp(id) => {
                if let Some(positon) = self.recipe.iter().position(|i| i.id == id) {
                    self.recipe.swap(positon, positon.saturating_sub(1));
                }
            }
            Message::MoveIngredientDown(id) => {
                if let Some(positon) = self.recipe.iter().position(|i| i.id == id) {
                    if self.recipe.len() != positon + 1 {
                        self.recipe.swap(positon, positon + 1);
                    }
                }
            }
            Message::RemoveIngredient(id) => {
                self.recipe.retain(|i| i.id != id);
            }
            Message::SelectIngredient(id) => {}
            Message::SelectIngredientPreview(id) => {
                for ingredient in &mut self.ingredient_list() {
                    if ingredient.id == id {
                        ingredient.toggle_selected();
                    } else {
                        ingredient.set_selected(false);
                    }
                }
            }
            Message::IngredientOutputChangeType(id) => {
                if let Some(ingredient) = self.recipe.iter_mut().find(|i| i.id == id) {
                    ingredient.toggle_output_type();
                }
            }
            Message::IngredientOutputChange(id, msg) => {
                if let Some(ingredient) = self.recipe.iter_mut().find(|i| i.id == id) {
                    ingredient.set_output(msg);
                }
            }
            Message::IngredientDataChange(id, msg) => {
                if let Some(ingredient) = self.recipe.iter_mut().find(|i| i.id == id) {
                    ingredient.set_input(msg);
                }
            }
            Message::RunAll => {
                self.state.as_mut().unwrap().output = String::new();
                for ingredient in &self.recipe {
                    if let Err(e) = ingredient.run(&mut self.state.as_mut().unwrap()) {
                        debug!("Error occured: '{:?}'. Restarting...", e);
                        self.state
                            .as_mut()
                            .unwrap()
                            .program
                            .restart()
                            .expect("Unable to restart program");
                        break;
                    }
                }
            }
            Message::CreateRegister(id) => {
                if let Some(ingredient) = self.recipe.iter_mut().find(|i| i.id == id) {
                    self.state
                        .as_mut()
                        .unwrap()
                        .registers
                        .set(&ingredient.output, vec![]);
                    ingredient.toggle_output_type();
                }
            }
            Message::SaveRecipe => {
                let path = format!("recipes/{}", self.save_recipe_name);
                let file = File::create(&path).unwrap();
                let serialized = serde_json::to_string(&self.recipe).unwrap();
                fs::write(&path, &serialized).expect("Unable to write file");
            }
            Message::SaveIngredient => {
                let path = format!("ingredients/{}", self.save_recipe_name);
                let file = File::create(&path).unwrap();
                let serialized = serde_json::to_string(&self.recipe).unwrap();
                fs::write(&path, &serialized).expect("Unable to write file");

                self.load_custom_ingredients();
            }
            Message::LoadRecipe => {
                let path = format!("recipes/{}", self.load_recipe_name);
                let data = std::fs::read_to_string(&path).expect("Unable to read file");
                let deserialized = serde_json::from_str(&data);
                self.recipe = deserialized.unwrap();
                debug!("Loaded recipe {}", self.load_recipe_name);

                for ingredient in &self.recipe {
                    self.state.as_mut().unwrap().registers.set(&ingredient.output, vec![]);
                }
            }
            Message::SaveRecipeChanged(msg) => {
                self.save_recipe_name = msg;
            }
            Message::LoadRecipeChanged(msg) => {
                self.load_recipe_name = msg;
            }
        };

        self.load_log();
        Command::none()
    }

    fn should_exit(&self) -> bool {
        self.should_exit
    }

    fn view(&mut self) -> Element<Message> {
        match self.current_scene {
            Scene::ChooseProgram => self.view_choose_program(),
            Scene::Recipe => self.view_recipe(),
        }
    }
}
