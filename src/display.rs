use rustbox::RustBox;
use rustbox::Event;
use rustbox::Color;
use rustbox;
use std::char;
use filedata::FileData;
use std::collections::VecDeque;
use std::path::Path;

pub const COLOR: Color = Color::Yellow;

pub struct Display
{
	running: bool, //is the app running?
	rustbox: RustBox, //rustbox instance
	data: FileData, //file buffer object
	width: usize, //screen width
	height: usize, //screen height
	line_scroll: usize, //line number of the first display line
	input_active: bool, //true -> write to commandline, false -> write to data buffer
	input: FileData, //commandline buffer object
	message_queue: VecDeque<String>, //list of messages to the user
	yn_question: Option<YNQuestion>, //if not none -> question to the user
	yn_question_state: bool //if yn_question is answered with yes or no, true -> yes
}

impl Display
{
	pub fn new(data: FileData) -> Display
	{
		let rbox = Display::init_rustbox();
		Display
		{
			running: true,
			rustbox: rbox,
			data: data,
			width: 0,
			height: 0,
			line_scroll: 0,
			input_active: false,
			input: FileData::new(),
			message_queue: VecDeque::new(),
			yn_question: None,
			yn_question_state: false
		}
	}

	fn init_rustbox() -> RustBox
	{
		match RustBox::init(Default::default())
		{
			Result::Ok(v) => v,
			Result::Err(_) => {panic!();}
		}
	}

	//poll events, repaint after resize or key press
	pub fn run(mut self)
	{
		self.width = self.rustbox.width();
		self.height = self.rustbox.height();
		self.draw();
		loop
		{
			match self.rustbox.poll_event(true)
			{
				Ok(Event::KeyEventRaw(_, key, charval)) =>
				{
					match char::from_u32(charval)
					{
						Some(character) => {
							self.key_event(key, character);
							self.draw();
						},
						None => {}
					};
					if self.running == false
					{
						break;
					}
				},
				Ok(Event::ResizeEvent(width, height)) =>
				{
					self.resize_event(width as usize, height as usize);
					self.draw();
				},
				Err(_) => {},
				_ => {}
			}
		}
	}

	//update dimension
	fn resize_event(&mut self, width: usize, height: usize)
	{
		self.width = width;
		self.height = height;
	}

	//handle incoming events
	fn key_event(&mut self, key: u16, character: char)
	{
		match self.yn_question //capture input when question is asked
		{
			Some(_) => {
				self.question_key_event(key);
				return;
			},
			None => {}
		}

		if self.input_active && key == 13 //had to move here for ownership reasons
		{
			self.execute_input();
			return;
		}

		//ctrl keys
		if key == 14 //^N
		{
			self.execute_internal(String::from("new"));
		}
		else if key == 15 //^O
		{
			self.preset_input(String::from("open "));
		}
		else if key == 19 //^S
		{
			self.execute_internal(String::from("save"));
		}
		else if key == 17 //^Q
		{
			self.execute_internal(String::from("quit"));
		}
		else if key == 23 //^W
		{
			self.preset_input(String::from("save "));
		}

		//vertical scrolling
		if self.data.get_cursor_line() < self.line_scroll
		{
			self.line_scroll -= 1;
		}
		if self.data.get_cursor_line() >= self.line_scroll + self.height - 1
		{
			self.line_scroll += 1;
		}

		//match pressed key
		let in_active = self.input_active;
		let mod_data = match in_active
		{
			false => &mut self.data,
			true => &mut self.input
		};
		match key
		{
			65517 => { //up
				if !self.input_active
				{
					mod_data.move_cursor_up();
				}
			},
			65515 => { //left
				mod_data.move_cursor_left();
			},
			65516 => { //down
				if !self.input_active
				{
					mod_data.move_cursor_down();
				}
			},
			65514 => { //right
				mod_data.move_cursor_right();
			},
			127 => { //bsp
				mod_data.backspace();
			},
			65522 => { //remove
				mod_data.remove();
			},
			9 => { //tab
				if !self.input_active
				{
					mod_data.write_char('\t');
				}
			},
			13 => { //enter, other half moved to start of function
				mod_data.enter();
			},
			27 => { //esc
				self.input_active = !self.input_active;
				if self.input_active == false //condition for mod_data = self.input
				{
					mod_data.clear();
				}
			},
			32 => {//space
				mod_data.write_char(' '); //doesnt get recognized??
			},
			_ => {
				if character.is_alphabetic() || character.len_utf8() == 1 //filter out ^.. chars and others
				{
					mod_data.write_char(character);
				}
			}
		}
	}

	//key handle if the user is being asked a question
	fn question_key_event(&mut self, key: u16)
	{
		if key == 65515 || key == 65514 //left or right
		{
			self.yn_question_state = !self.yn_question_state;
		}
		if key == 13
		{
			self.question_answered();
		}
	}

	//called when the user confirms an answer (enter)
	//if the user confirms the action it executes what should have been executed before,
	//this action is stored in self.yn_question.option
	fn question_answered(&mut self)
	{
		let mut notification_vec = Vec::new();
		{
			let answer = self.yn_question_state.clone();
			let question = match self.yn_question
			{
				None => {return;},
				Some(ref v) => {
					v
				}
			};
			let ref option = question.option;
			match *option
			{
				YNOption::NewIgnoreModified => {
					if answer == true
					{
						self.data.clear();
					}
				},
				YNOption::OpenIgnoreModified(ref path) => {
					if answer == true
					{
						match self.data.open(path.clone())
			    		{
			    			Ok(_) => {
			    				notification_vec.push(format!("opened"));
			    			},
			    			Err(e) => {
			    				notification_vec.push(format!("error: {}", e));
			    			}
			    		}
					}
				},
				YNOption::SaveIgnoreExisting(ref path) => {
					if answer == true
					{
						match self.data.save_to(path.clone())
						{
							Ok(_) => {
								notification_vec.push(format!("saved"));
							},
							Err(e) => {
								notification_vec.push(format!("error: {}", e));
							}
						}
					}
				},
				YNOption::QuitIgnoreModified => {
					if answer == true
					{
						self.running = false;
					}
				}
			}
		}
		for notification in notification_vec
		{
			self.notify(notification);
		}
		self.yn_question = None;
	}

	fn draw(&mut self)
	{
		self.rustbox.clear();
		self.draw_text();
		self.draw_title();
		self.draw_question();
		self.draw_message();
		self.rustbox.present();
	}

	//pull a message from queue and display it
	fn draw_message(&mut self)
	{
		if self.message_queue.is_empty() == false
		{
			let message = self.message_queue.pop_back().unwrap();
			let pos_x = self.width/2-message.len()/2;
			self.rustbox.print(pos_x, self.height-1, rustbox::RB_NORMAL, Color::Black, COLOR, &format!("{}", message));
		}
	}

	//draw a box displaying the question to the user
	fn draw_question(&self)
	{
		let option = match self.yn_question
		{
			None => {return;},
			Some(ref v) => v
		};
		let box_width = option.text.len()+2;
		let box_x = self.width/2-box_width/2;
		let box_y = self.height/2-2;
		self.fill_rect(box_x, box_y, box_width, 5);
		self.rustbox.print(box_x+1, box_y+1, rustbox::RB_BOLD, Color::Black, Color::White, &option.text);
		if self.yn_question_state == true
		{
			self.rustbox.print(box_x+box_width/2-7, box_y+3, rustbox::RB_BOLD, Color::Black, COLOR, &format!("<YES>"));
			self.rustbox.print(box_x+box_width/2+3, box_y+3, rustbox::RB_BOLD, Color::Black, Color::White, &format!("<NO>"));
		}
		else 
		{
		    self.rustbox.print(box_x+box_width/2-7, box_y+3, rustbox::RB_BOLD, Color::Black, Color::White, &format!("<YES>"));
			self.rustbox.print(box_x+box_width/2+3, box_y+3, rustbox::RB_BOLD, Color::Black, COLOR, &format!("<NO>"));
		}
	}

	//fill a white rectangle on screen
	fn fill_rect(&self, x: usize, y: usize, width: usize, height: usize)
	{
		let line = pad_to(String::new(), width);//create line string
		for pos_y in y .. (height+y)
		{
			self.rustbox.print(x, pos_y, rustbox::RB_NORMAL, Color::White, Color::White, &line);
		}
	}

	//draw status bar/commandline
	fn draw_title(&self)
	{
		let mut title = String::new();
		if self.input_active
		{
			let input_text = self.input.to_string_copy();
			title.push_str(&input_text.trim());
			self.rustbox.set_cursor(self.input.get_cursor_char() as isize, 0);
		}
		else 
		{
			if self.data.is_modified()
			{
				title.push('~');
			}
			title.push_str(&self.data.get_title());
			let cursor_pos_text = format!("  [{},{}]  lines: {}", self.data.get_cursor_line()+1, self.data.get_cursor_char()+1, self.data.get_lines());
			title.push_str(&cursor_pos_text);
		}	
		self.rustbox.print(0, 0, rustbox::RB_NORMAL, Color::Black, COLOR, &pad_to(title, self.width));
	}

	//draw the editor pane
	//differentiates between the on-screen and in-data position of the cursor
	//cur_line_data, cur_char_data -> data pointer position
	//cur_line, cur_char -> display pointer position
	fn draw_text(&mut self)
	{
		let line_no_len = self.data.get_line_number_len();
		let mut cur_line = 1;
		let mut cur_line_data = self.line_scroll;
		while cur_line < self.height
		{
			let mut cur_char = line_no_len+3;
			let mut cur_char_data = 0;
			let line_content = match self.data.get_line(cur_line_data)
			{
				None => {break;},
				Some(v) => v
			};
			let mut line_str = pad_to(format!("{} ", cur_line_data+1), line_no_len+1);
			line_str.push('|');
			self.rustbox.print(0, cur_line, rustbox::RB_NORMAL, COLOR, Color::Default, &line_str);
			for ch in line_content
			{
				if cur_char_data == self.data.get_cursor_char() && cur_line_data == self.data.get_cursor_line()
				{
					self.rustbox.set_cursor(cur_char as isize, cur_line as isize);
				}
				if ch != &'\t'
				{
					self.rustbox.print(cur_char, cur_line, rustbox::RB_NORMAL, Color::White, Color::Default, &format!("{}",ch));
				}
				else 
				{
				    cur_char += 3;
				}
				cur_char += 1;
				cur_char_data += 1;
				if cur_char >= self.width //wrap lines if they reach the end of the line
				{
					cur_line += 1;
					cur_char = line_no_len+7;
					self.rustbox.print(line_no_len+3, cur_line, rustbox::RB_NORMAL, COLOR, Color::Default, &format!("-->"));
				}
			}
			if cur_char_data == self.data.get_cursor_char() && cur_line_data == self.data.get_cursor_line()
			{
				self.rustbox.set_cursor(cur_char as isize, cur_line as isize);
			}
			cur_line += 1;
			cur_line_data += 1;
		}
	}

	//open the commandline with a preset command
	fn preset_input(&mut self, command: String)
	{
		self.input_active = true;
		self.input.clear();
		for ch in command.chars()
		{
			self.input.write_char(ch);
		}
	}

	//execute the command entered in the commandline
	fn execute_input(&mut self)
	{
		let command = self.input.to_string_copy();
		self.execute_internal(command);
	}

	//match command + execute
	fn execute_internal(&mut self, mut command: String)
	{
		command = command.trim().to_owned();
		self.input.clear();
		self.input_active = false;
		let mut split_iter = command.split(' ');
		let op: String = match split_iter.next()
		{
			Some(v) => v.to_owned(),
			None => {
				self.notify(format!("error: no input"));
				return;
			}
		};
		if &op == "new"
		{
			if self.data.is_modified()
			{
				self.create_yn_req(YNOption::NewIgnoreModified);
			}
			else 
			{
			    self.data.clear();
			}
		}
		else if &op == "save"
		{
			//save current file if no arg was given
			let path = match split_iter.next()
			{
				Some(v) => v.to_owned(),
				None => {
					if self.data.get_path() == None
					{
						self.notify(format!("error: file is unnamed"))
					}
					else 
					{
						match self.data.save()
						{
							Ok(_) => {
								self.notify(format!("saved"))
							},
							Err(e) => {
								self.notify(format!("error: {}", e));
							}
						}
					}
					return;
				}
			};
			match self.data.get_path()
			{
				Some(v) => {
					if v != path
					{
						let path_copy = path.clone();
						let new_file = Path::new(&path_copy);
						if new_file.exists() == true
						{
							self.create_yn_req(YNOption::SaveIgnoreExisting(path));
							return;
						}
					}
				},
				None => {}
			}
			match self.data.save_to(path.clone())
			{
				Ok(_) => {
					self.data.set_path(Some(path));
					self.notify(format!("saved"));
				},
				Err(e) => self.notify(format!("error: {}", e))
			}
		}
		else if &op == "open"
		{
			let path = match split_iter.next()
			{
				Some(v) => v.to_owned(),
				None => {
					self.notify(format!("error: usage: open <file>"));
					return;
				}
			};
			if self.data.is_modified()
			{
				self.create_yn_req(YNOption::OpenIgnoreModified(path));
			}
			else 
			{
			    match self.data.open(path)
			    {
			    	Ok(_) => {
			    		self.notify(format!("opened"));
			    	},
			    	Err(e) => {self.notify(format!("error: {}", e))}
			    }
			}
		}
		else if &op == "quit"
		{
			if self.data.is_modified()
			{
				self.create_yn_req(YNOption::QuitIgnoreModified);
			}
			else 
			{
			    self.running = false;
			}
		}
		else 
		{
		    self.notify(format!("error: unknown command: {}", op));
		}
	}

	//setup a question
	fn create_yn_req(&mut self, option: YNOption)
	{
		let message = match option
		{
			YNOption::NewIgnoreModified => format!("unsaved changes! continue?"),
			YNOption::OpenIgnoreModified(_) => format!("unsaved changes! continue?"),
			YNOption::SaveIgnoreExisting(_) => format!("file already exists! continue?"),
			YNOption::QuitIgnoreModified => format!("unsaved changes! continue")
		};
		self.yn_question = Some(YNQuestion::new(message, option));
		self.yn_question_state = false;
	}

	//push a message to the message queue to be displayed later
	fn notify(&mut self, message: String)
	{
		self.message_queue.push_front(message);
	}
}

//widen the string with spaces
fn pad_to(mut string: String, width: usize) -> String
{
	if string.len() < width
	{
		for _ in 0..width-string.len()
		{
			string.push(' ');
		}
	}
	string
}

enum YNOption
{
	NewIgnoreModified, //when the user wants to create a new file, but the current one is unsaved
	OpenIgnoreModified(String),//String -> path, like NewIgnoreModified, but after opening a file
	SaveIgnoreExisting(String),//String -> path, when the user wants to write to an existing file that is NOT the original file
	QuitIgnoreModified //when the user wants to exit, but the current file is unsaved
}

struct YNQuestion
{
	pub text: String, //the displayed text
	pub option: YNOption //the questions origin
}

impl YNQuestion
{
	pub fn new(text: String, option: YNOption) -> YNQuestion
	{
		YNQuestion
		{
			text: text,
			option: option
		}
	}
}