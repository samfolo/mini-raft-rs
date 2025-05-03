#![allow(unused)] // TODO: use... need to refactor something first.

use std::fmt;

#[derive(Debug, PartialEq)]
pub struct InMemoryStateMachine {
    x: i64,
    y: i64,
    z: i64,
}

impl InMemoryStateMachine {
    pub fn new() -> Self {
        Self { x: 0, y: 0, z: 0 }
    }

    fn get(&self, key: StateKey) -> i64 {
        match key {
            StateKey::X => self.x,
            StateKey::Y => self.y,
            StateKey::Z => self.z,
        }
    }

    fn set(&mut self, key: StateKey, new_value: i64) {
        match key {
            StateKey::X => self.x = new_value,
            StateKey::Y => self.y = new_value,
            StateKey::Z => self.z = new_value,
        }
    }

    pub fn get_snapshot(&self) -> InMemoryStateMachineSnapshot {
        InMemoryStateMachineSnapshot {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }

    pub fn apply_command(&mut self, command: &Command) {
        command.exec(self)
    }
}

impl Default for InMemoryStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq<InMemoryStateMachineSnapshot> for InMemoryStateMachine {
    fn eq(&self, other: &InMemoryStateMachineSnapshot) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}

/// StateKey represents the location of the target state client request was made to update.
#[derive(Debug, Clone, Copy)]
pub enum StateKey {
    X,
    Y,
    Z,
}

impl fmt::Display for StateKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                StateKey::X => "State.x",
                StateKey::Y => "State.y",
                StateKey::Z => "State.z",
            }
        )
    }
}

/// Op represents the operation to be taken on the target state.
#[derive(Debug, Clone, Copy)]
pub enum Op {
    Increment,
    Decrement,
    Replace,
}

impl Op {
    fn exec(&self, lhs: i64, rhs: i64) -> i64 {
        match self {
            Op::Increment => lhs.saturating_add(rhs),
            Op::Decrement => lhs.saturating_sub(rhs),
            Op::Replace => rhs,
        }
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Op::Increment => "+=",
                Op::Decrement => "-=",
                Op::Replace => "<=",
            }
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Command {
    op: Op,
    key: StateKey,
    value: i64,
}

impl Command {
    pub fn new(op: Op, key: StateKey, value: i64) -> Self {
        Self { op, key, value }
    }

    fn op(&self) -> Op {
        self.op
    }
    fn key(&self) -> StateKey {
        self.key
    }

    fn value(&self) -> i64 {
        self.value
    }

    fn exec(&self, state_machine: &mut InMemoryStateMachine) {
        let new_value = self.op().exec(state_machine.get(self.key()), self.value());
        state_machine.set(self.key(), new_value);
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{ command: [ State.{:?} {} {} ] }}",
            self.key(),
            self.op(),
            self.value()
        )
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct InMemoryStateMachineSnapshot {
    x: i64,
    y: i64,
    z: i64,
}

impl fmt::Display for InMemoryStateMachineSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{ X => {}, Y => {}, Z => {} }}", self.x, self.y, self.z)
    }
}

impl PartialEq<InMemoryStateMachine> for InMemoryStateMachineSnapshot {
    fn eq(&self, other: &InMemoryStateMachine) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Command, InMemoryStateMachine,
        Op::{self, *},
        StateKey::{self, *},
    };

    // ---------------------------------------------
    fn command(op: Op, key: StateKey, value: i64) -> Command {
        Command { op, key, value }
    }

    // ---------------------------------------------
    fn run(
        mut actual: InMemoryStateMachine,
        commands: &[Command],
        expected: InMemoryStateMachine,
    ) -> anyhow::Result<()> {
        commands.iter().for_each(|cmd| actual.apply_command(cmd));

        assert_eq!(expected, actual);

        Ok(())
    }

    // ---------------------------------------------
    #[test]
    fn applies_increment_commands() -> anyhow::Result<()> {
        run(
            InMemoryStateMachine { x: 0, y: 0, z: 0 },
            &[
                command(Increment, X, 5),
                command(Increment, Z, 15),
                command(Increment, X, 5),
                command(Increment, Z, 10),
                command(Increment, Y, 2),
                command(Increment, Z, 4),
                command(Increment, Y, 3),
                command(Increment, Y, 15),
                command(Increment, Z, 1),
            ],
            InMemoryStateMachine {
                x: 10,
                y: 20,
                z: 30,
            },
        )
    }

    // ---------------------------------------------
    #[test]
    fn applies_decrement_commands() -> anyhow::Result<()> {
        run(
            InMemoryStateMachine {
                x: 1000,
                y: 1000,
                z: 1000,
            },
            &[
                command(Decrement, X, 125),
                command(Decrement, Z, 100),
                command(Decrement, Z, 100),
                command(Decrement, Y, 900),
                command(Decrement, Z, 100),
                command(Decrement, X, 150),
                command(Decrement, X, 25),
                command(Decrement, Z, 100),
                command(Decrement, Y, 99),
                command(Decrement, Z, 100),
            ],
            InMemoryStateMachine {
                x: 700,
                y: 1,
                z: 500,
            },
        )
    }

    // ---------------------------------------------
    #[test]
    fn applies_replace_commands() -> anyhow::Result<()> {
        run(
            InMemoryStateMachine {
                x: 42,
                y: 42,
                z: 42,
            },
            &[
                command(Replace, X, 9),
                command(Replace, Y, 18),
                command(Replace, Z, 127),
                command(Replace, X, 6),
                command(Replace, Y, -4),
            ],
            InMemoryStateMachine {
                x: 6,
                y: -4,
                z: 127,
            },
        )
    }

    // ---------------------------------------------
    #[test]
    fn applies_mixed_commands() -> anyhow::Result<()> {
        run(
            InMemoryStateMachine { x: 0, y: 0, z: 0 },
            &[
                command(Increment, Y, 2),
                command(Increment, X, 1),
                command(Increment, Z, 3),
                command(Replace, Y, 16),
                command(Decrement, X, 10),
                command(Increment, Z, 5),
                command(Decrement, Y, 1),
                command(Decrement, Z, 103),
            ],
            InMemoryStateMachine {
                x: -9,
                y: 15,
                z: -95,
            },
        )
    }

    // ---------------------------------------------
    #[test]
    fn applies_commands_without_integer_overflow() -> anyhow::Result<()> {
        run(
            InMemoryStateMachine {
                x: i64::MIN,
                y: i64::MAX,
                z: 1,
            },
            &[
                command(Decrement, X, 10),
                command(Increment, Y, 1),
                command(Increment, Z, i64::MAX),
            ],
            InMemoryStateMachine {
                x: i64::MIN,
                y: i64::MAX,
                z: i64::MAX,
            },
        )
    }
}
