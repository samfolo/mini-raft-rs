mod command;

use std::fmt;

#[derive(Debug, PartialEq)]
pub struct StateMachine {
    x: i64,
    y: i64,
    z: i64,
}

impl StateMachine {
    fn new() -> Self {
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

    pub fn apply_command<T: Command>(&mut self, command: &T) -> anyhow::Result<()> {
        command.exec(self)
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
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

pub trait Command {
    fn op(&self) -> Op;
    fn key(&self) -> StateKey;
    fn value(&self) -> i64;

    fn exec(&self, state_machine: &mut StateMachine) -> anyhow::Result<()> {
        let new_value = self.op().exec(state_machine.get(self.key()), self.value());
        state_machine.set(self.key(), new_value);

        Ok(())
    }
}

impl fmt::Display for dyn Command {
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

#[cfg(test)]
mod tests {
    use super::{
        Command,
        Op::{self, *},
        StateKey::{self, *},
        StateMachine,
    };

    // ---------------------------------------------
    struct TestCommand {
        op: Op,
        key: StateKey,
        value: i64,
    }

    impl Command for TestCommand {
        fn key(&self) -> StateKey {
            self.key
        }

        fn op(&self) -> Op {
            self.op
        }

        fn value(&self) -> i64 {
            self.value
        }
    }

    // ---------------------------------------------
    fn command(op: Op, key: StateKey, value: i64) -> TestCommand {
        TestCommand { op, key, value }
    }

    // ---------------------------------------------
    fn run(
        mut actual: StateMachine,
        commands: &[TestCommand],
        expected: StateMachine,
    ) -> anyhow::Result<()> {
        commands
            .iter()
            .try_for_each(|cmd| actual.apply_command(cmd))?;

        assert_eq!(expected, actual);

        Ok(())
    }

    // ---------------------------------------------
    #[test]
    fn applies_increment_commands() -> anyhow::Result<()> {
        run(
            StateMachine { x: 0, y: 0, z: 0 },
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
            StateMachine {
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
            StateMachine {
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
            StateMachine {
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
            StateMachine {
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
            StateMachine {
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
            StateMachine { x: 0, y: 0, z: 0 },
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
            StateMachine {
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
            StateMachine {
                x: i64::MIN,
                y: i64::MAX,
                z: 1,
            },
            &[
                command(Decrement, X, 10),
                command(Increment, Y, 1),
                command(Increment, Z, i64::MAX),
            ],
            StateMachine {
                x: i64::MIN,
                y: i64::MAX,
                z: i64::MAX,
            },
        )
    }
}
