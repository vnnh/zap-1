use crate::{
	config::{Config, EvCall, EvDecl, EvSource, EvType, TyDecl},
	irgen::{des, ser},
};

use super::Output;

struct ServerOutput<'src> {
	config: &'src Config<'src>,
	tabs: u32,
	buf: String,
}

impl<'a> Output for ServerOutput<'a> {
	fn push(&mut self, s: &str) {
		self.buf.push_str(s);
	}

	fn indent(&mut self) {
		self.tabs += 1;
	}

	fn dedent(&mut self) {
		self.tabs -= 1;
	}

	fn push_indent(&mut self) {
		for _ in 0..self.tabs {
			self.push("\t");
		}
	}
}

impl<'a> ServerOutput<'a> {
	pub fn new(config: &'a Config) -> Self {
		Self {
			config,
			tabs: 0,
			buf: String::new(),
		}
	}

	fn push_tydecl(&mut self, tydecl: &TyDecl) {
		let name = &tydecl.name;
		let ty = &tydecl.ty;

		self.push_indent();
		self.push(&format!("export type {name} = "));
		self.push_ty(ty);
		self.push("\n");

		self.push_line(&format!("function types.write_{name}(value: {name})"));
		self.indent();
		self.push_stmts(&ser::gen(ty, "value", self.config.write_checks));
		self.dedent();
		self.push_line("end");

		self.push_line(&format!("function types.read_{name}()"));
		self.indent();
		self.push_line("local value;");
		self.push_stmts(&des::gen(ty, "value", true));
		self.push_line("return value");
		self.dedent();
		self.push_line("end");
	}

	fn push_tydecls(&mut self) {
		for tydecl in self.config.tydecls.iter() {
			self.push_tydecl(tydecl);
		}
	}

	fn push_reliable_header(&mut self) {
		self.push_line("reliable.OnServerEvent:Connect(function(player, buf, inst)");
		self.indent();
		self.push_line("incoming_buf = buf");
		self.push_line("incoming_inst = inst");
		self.push_line("incoming_read = 0");

		self.push_line("local len = bufer.len(buf)");
		self.push_line("while incoming_read < len do");

		self.indent();

		self.push_line(&format!(
			"local id = bufer.read{}(buf, read({}))",
			self.config.event_id_ty(),
			self.config.event_id_ty().size()
		));
	}

	fn push_reliable_callback(&mut self, first: bool, ev: &EvDecl, id: usize) {
		self.push_indent();

		if first {
			self.push("if ");
		} else {
			self.push("elseif ");
		}

		// push_line is not used here as indent was pushed above
		// and we don't want to push it twice, especially after
		// the if/elseif
		self.push(&format!("id == {id} then"));
		self.push("\n");

		self.indent();

		self.push_line("local value");
		self.push_stmts(&des::gen(&ev.data, "value", true));

		match ev.call {
			EvCall::SingleSync => self.push_line(&format!("if events[{id}] then events[{id}](player, value) end")),
			EvCall::SingleAsync => self.push_line(&format!(
				"if events[{id}] then task.spawn(events[{id}], player, value) end"
			)),
			EvCall::ManySync => self.push_line(&format!("for _, cb in events[{id}] do cb(player, value) end")),
			EvCall::ManyAsync => self.push_line(&format!(
				"for _, cb in events[{id}] do task.spawn(cb, player, value) end"
			)),
		}

		self.dedent();
	}

	fn push_reliable_footer(&mut self) {
		self.push_line("else");
		self.indent();
		self.push_line("error(\"Unknown event id\")");
		self.dedent();
		self.push_line("end");
		self.dedent();
		self.push_line("end");
		self.dedent();
		self.push_line("end)");
	}

	fn push_reliable(&mut self) {
		self.push_reliable_header();

		let mut first = true;

		for (i, ev) in self
			.config
			.evdecls
			.iter()
			.enumerate()
			.filter(|(_, ev_decl)| ev_decl.from == EvSource::Client && ev_decl.evty == EvType::Reliable)
		{
			let id = i + 1;

			self.push_reliable_callback(first, ev, id);
			first = false;
		}

		self.push_reliable_footer();
	}

	fn push_unreliable_header(&mut self) {
		self.push_line("unreliable.OnServerEvent:Connect(function(player, buf, inst)");
		self.indent();
		self.push_line("incoming_buf = buf");
		self.push_line("incoming_inst = inst");
		self.push_line("incoming_read = 0");

		self.push_line(&format!(
			"local id = bufer.read{}(buf, read({}))",
			self.config.event_id_ty(),
			self.config.event_id_ty().size()
		));
	}

	fn push_unreliable_callback(&mut self, first: bool, ev: &EvDecl, id: usize) {
		self.push_indent();

		if first {
			self.push("if ");
		} else {
			self.push("elseif ");
		}

		// push_line is not used here as indent was pushed above
		// and we don't want to push it twice, especially after
		// the if/elseif
		self.push(&format!("id == {id} then"));
		self.push("\n");

		self.indent();

		self.push_line("local value");
		self.push_stmts(&des::gen(&ev.data, "value", true));

		match ev.call {
			EvCall::SingleSync => self.push_line(&format!("if events[{id}] then events[{id}](player, value) end")),
			EvCall::SingleAsync => self.push_line(&format!(
				"if events[{id}] then task.spawn(events[{id}], player, value) end"
			)),
			EvCall::ManySync => self.push_line(&format!("for _, cb in events[{id}] do cb(player, value) end")),
			EvCall::ManyAsync => self.push_line(&format!(
				"for _, cb in events[{id}] do task.spawn(cb, player, value) end"
			)),
		}

		self.dedent();
	}

	fn push_unreliable_footer(&mut self) {
		self.push_line("else");
		self.indent();
		self.push_line("error(\"Unknown event id\")");
		self.dedent();
		self.push_line("end");
		self.dedent();
		self.push_line("end)");
	}

	fn push_unreliable(&mut self) {
		self.push_unreliable_header();

		let mut first = true;

		for (i, ev) in self
			.config
			.evdecls
			.iter()
			.enumerate()
			.filter(|(_, ev_decl)| ev_decl.from == EvSource::Client && ev_decl.evty == EvType::Unreliable)
		{
			let id = i + 1;

			self.push_unreliable_callback(first, ev, id);
			first = false;
		}

		self.push_unreliable_footer();
	}

	fn push_callback_lists(&mut self) {
		self.push_line(&format!("local events = table.create({})", self.config.evdecls.len()));

		for (i, _) in self.config.evdecls.iter().enumerate().filter(|(_, ev_decl)| {
			ev_decl.from == EvSource::Client && matches!(ev_decl.call, EvCall::ManyAsync | EvCall::ManySync)
		}) {
			let id = i + 1;

			self.push_line(&format!("events[{id}] = {{}}"));
		}
	}

	fn push_write_event_id(&mut self, id: usize) {
		self.push_line(&format!("local pos = alloc({})", self.config.event_id_ty().size()));
		self.push_line(&format!(
			"bufer.write{}(outgoing_buf, pos, {id})",
			self.config.event_id_ty()
		));
	}

	fn push_return_fire(&mut self, ev: &EvDecl, id: usize) {
		let ty = &ev.data;

		let fire = self.config.casing.with("Fire", "fire", "fire");
		let player = self.config.casing.with("Player", "player", "player");
		let value = self.config.casing.with("Value", "value", "value");

		self.push_indent();
		self.push(&format!("{fire} = function({player}: Player, {value}: "));
		self.push_ty(ty);
		self.push(")\n");
		self.indent();

		match ev.evty {
			EvType::Reliable => self.push_line(&format!("load(player_map[{player}])")),
			EvType::Unreliable => self.push_line("load_empty()"),
		}

		self.push_write_event_id(id);

		self.push_stmts(&ser::gen(ty, value, self.config.write_checks));

		match ev.evty {
			EvType::Reliable => self.push_line(&format!("player_map[{player}] = save()")),
			EvType::Unreliable => {
				self.push_line("local buf = bufer.create(outgoing_used)");
				self.push_line("bufer.copy(buf, 0, outgoing_buf, 0, outgoing_used)");
				self.push_line(&format!("unreliable:FireClient({player}, buf, outgoing_inst)"));
			}
		}

		self.dedent();
		self.push_line("end,");
	}

	fn push_return_fire_all(&mut self, ev: &EvDecl, id: usize) {
		let ty = &ev.data;

		let fire_all = self.config.casing.with("FireAll", "fireAll", "fire_all");
		let value = self.config.casing.with("Value", "value", "value");

		self.push_indent();
		self.push(&format!("{fire_all} = function({value}: "));
		self.push_ty(ty);
		self.push(")\n");
		self.indent();

		self.push_line("load_empty()");

		self.push_write_event_id(id);

		self.push_stmts(&ser::gen(ty, value, self.config.write_checks));

		match ev.evty {
			EvType::Reliable => {
				self.push_line("local buf, used, inst = outgoing_buf, outgoing_used, outgoing_inst");
				self.push_line("for player, outgoing in player_map do");
				self.indent();
				self.push_line("load(outgoing)");
				self.push_line("local pos = alloc(used)");
				self.push_line("bufer.copy(outgoing_buf, pos, buf, 0, used)");
				self.push_line("player_map[player] = save()");
				self.dedent();
				self.push_line("end");
			}

			EvType::Unreliable => {
				self.push_line("local buf = bufer.create(outgoing_used)");
				self.push_line("bufer.copy(buf, 0, outgoing_buf, 0, outgoing_used)");
				self.push_line("unreliable:FireAllClients(buf, outgoing_inst)")
			}
		}

		self.dedent();
		self.push_line("end,");
	}

	fn push_return_fire_except(&mut self, ev: &EvDecl, id: usize) {
		let ty = &ev.data;

		let fire_except = self.config.casing.with("FireExcept", "fireExcept", "fire_except");
		let except = self.config.casing.with("Except", "except", "except");
		let value = self.config.casing.with("Value", "value", "value");

		self.push_indent();
		self.push(&format!("{fire_except} = function({except}: Player, {value}: "));
		self.push_ty(ty);
		self.push(")\n");
		self.indent();

		self.push_line("load_empty()");

		self.push_write_event_id(id);

		self.push_stmts(&ser::gen(ty, value, self.config.write_checks));

		match ev.evty {
			EvType::Reliable => {
				self.push_line("local buf, used, inst = outgoing_buf, outgoing_used, outgoing_inst");
				self.push_line("for player, outgoing in player_map do");
				self.indent();
				self.push_line(&format!("if player ~= {except} then"));
				self.indent();
				self.push_line("load(outgoing)");
				self.push_line("local pos = alloc(used)");
				self.push_line("bufer.copy(outgoing_buf, pos, buf, 0, used)");
				self.push_line("player_map[player] = save()");
				self.dedent();
				self.push_line("end");
				self.dedent();
				self.push_line("end");
			}

			EvType::Unreliable => {
				self.push_line("local buf = bufer.create(outgoing_used)");
				self.push_line("bufer.copy(buf, 0, outgoing_buf, 0, outgoing_used)");
				self.push_line("for player in player_map do");
				self.indent();
				self.push_line(&format!("if player ~= {except} then"));
				self.indent();
				self.push_line("unreliable:FireClient(player, buf, outgoing_inst)");
				self.dedent();
				self.push_line("end");
				self.dedent();
				self.push_line("end");
			}
		}

		self.dedent();
		self.push_line("end,");
	}

	fn push_return_fire_list(&mut self, ev: &EvDecl, id: usize) {
		let ty = &ev.data;

		let fire_list = self.config.casing.with("FireList", "fireList", "fire_list");
		let list = self.config.casing.with("List", "list", "list");
		let value = self.config.casing.with("Value", "value", "value");

		self.push_indent();
		self.push(&format!("{fire_list} = function({list}: {{ Player }}, {value}: "));
		self.push_ty(ty);
		self.push(")\n");
		self.indent();

		self.push_line("load_empty()");

		self.push_write_event_id(id);

		self.push_stmts(&ser::gen(ty, value, self.config.write_checks));

		match ev.evty {
			EvType::Reliable => {
				self.push_line("local buf, used, inst = outgoing_buf, outgoing_used, outgoing_inst");
				self.push_line(&format!("for _, player in {list} do"));
				self.indent();
				self.push_line("load(player_map[player])");
				self.push_line("local pos = alloc(used)");
				self.push_line("bufer.copy(outgoing_buf, pos, buf, 0, used)");
				self.push_line("player_map[player] = save()");
				self.dedent();
				self.push_line("end");
			}

			EvType::Unreliable => {
				self.push_line("local buf = bufer.create(outgoing_used)");
				self.push_line("bufer.copy(buf, 0, outgoing_buf, 0, outgoing_used)");
				self.push_line(&format!("for _, player in {list} do"));
				self.indent();
				self.push_line("unreliable:FireClient(player, buf, outgoing_inst)");
				self.dedent();
				self.push_line("end");
			}
		}

		self.dedent();
		self.push_line("end,");
	}

	fn push_return_outgoing(&mut self) {
		for (i, ev) in self
			.config
			.evdecls
			.iter()
			.enumerate()
			.filter(|(_, ev_decl)| ev_decl.from == EvSource::Server)
		{
			let id = i + 1;

			self.push_line(&format!("{name} = {{", name = ev.name));
			self.indent();

			self.push_return_fire(ev, id);
			self.push_return_fire_all(ev, id);
			self.push_return_fire_except(ev, id);
			self.push_return_fire_list(ev, id);

			self.dedent();
			self.push_line("},");
		}
	}

	fn push_return_setcallback(&mut self, ev: &EvDecl, id: usize) {
		let ty = &ev.data;

		let set_callback = self.config.casing.with("SetCallback", "setCallback", "set_callback");
		let callback = self.config.casing.with("Callback", "callback", "callback");

		self.push_indent();
		self.push(&format!("{set_callback} = function({callback}: (Player, "));
		self.push_ty(ty);
		self.push(") -> ())\n");
		self.indent();

		self.push_line(&format!("events[{id}] = {callback}"));

		self.dedent();
		self.push_line("end,");
	}

	fn push_return_on(&mut self, ev: &EvDecl, id: usize) {
		let ty = &ev.data;

		let on = self.config.casing.with("On", "on", "on");
		let callback = self.config.casing.with("Callback", "callback", "callback");

		self.push_indent();
		self.push(&format!("{on} = function({callback}: (Player, "));
		self.push_ty(ty);
		self.push(") -> ())\n");
		self.indent();

		self.push_line(&format!("table.insert(events[{id}], {callback})"));

		self.dedent();
		self.push_line("end,");
	}

	pub fn push_return_listen(&mut self) {
		for (i, ev) in self
			.config
			.evdecls
			.iter()
			.enumerate()
			.filter(|(_, ev_decl)| ev_decl.from == EvSource::Client)
		{
			let id = i + 1;

			self.push_line(&format!("{name} = {{", name = ev.name));
			self.indent();

			match ev.call {
				EvCall::SingleSync | EvCall::SingleAsync => self.push_return_setcallback(ev, id),
				EvCall::ManySync | EvCall::ManyAsync => self.push_return_on(ev, id),
			}

			self.dedent();
			self.push_line("},");
		}
	}

	pub fn push_return(&mut self) {
		self.push_line("return {");
		self.indent();

		self.push_return_outgoing();
		self.push_return_listen();

		self.dedent();
		self.push_line("}");
	}

	pub fn output(mut self) -> String {
		self.push_file_header("Server");

		if self.config.evdecls.is_empty() {
			return self.buf;
		};

		self.push(include_str!("base.luau"));
		self.push(include_str!("server.luau"));

		self.push_tydecls();

		self.push_callback_lists();

		if self
			.config
			.evdecls
			.iter()
			.any(|ev| ev.evty == EvType::Reliable && ev.from == EvSource::Client)
		{
			self.push_reliable();
		}

		if self
			.config
			.evdecls
			.iter()
			.any(|ev| ev.evty == EvType::Unreliable && ev.from == EvSource::Client)
		{
			self.push_unreliable();
		}

		self.push_return();

		self.buf
	}
}

pub fn code(config: &Config) -> String {
	ServerOutput::new(config).output()
}