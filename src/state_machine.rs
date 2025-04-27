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

    fn apply_command<T: Command>(&mut self, command: &T) -> anyhow::Result<()> {
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
    use super::*;

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
    fn run(commands: &[TestCommand], expected: StateMachine) -> anyhow::Result<()> {
        let mut actual = StateMachine::default();
        commands
            .iter()
            .try_for_each(|cmd| actual.apply_command(cmd));

        assert_eq!(expected, actual);

        Ok(())
    }

    // ---------------------------------------------

    #[test]
    fn applies_increment_commands() -> anyhow::Result<()> {
        run(
            &[
                TestCommand {
                    op: Op::Increment,
                    key: StateKey::X,
                    value: 5,
                },
                TestCommand {
                    op: Op::Increment,
                    key: StateKey::Z,
                    value: 15,
                },
                TestCommand {
                    op: Op::Increment,
                    key: StateKey::X,
                    value: 5,
                },
                TestCommand {
                    op: Op::Increment,
                    key: StateKey::Z,
                    value: 10,
                },
                TestCommand {
                    op: Op::Increment,
                    key: StateKey::Y,
                    value: 2,
                },
                TestCommand {
                    op: Op::Increment,
                    key: StateKey::Z,
                    value: 4,
                },
                TestCommand {
                    op: Op::Increment,
                    key: StateKey::Y,
                    value: 3,
                },
                TestCommand {
                    op: Op::Increment,
                    key: StateKey::Y,
                    value: 15,
                },
                TestCommand {
                    op: Op::Increment,
                    key: StateKey::Z,
                    value: 1,
                },
            ],
            StateMachine {
                x: 10,
                y: 20,
                z: 30,
            },
        )
    }
}
