use csv::Trim;
use rust_decimal::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashMap, env, error::Error, io, process};

#[derive(Debug, Deserialize)]
pub struct Transaction {
    // I could either escape type like r#type or rename it bc it's a reserved word
    #[serde(rename = "type")]
    r_type: String,
    client: u16,
    tx: u32,
    #[serde(deserialize_with = "four_precision_deserializer")]
    amount: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Account {
    client: u16,
    #[serde(serialize_with = "four_precision_serializer")]
    available: f64,
    #[serde(serialize_with = "four_precision_serializer")]
    held: f64,
    #[serde(serialize_with = "four_precision_serializer")]
    total: f64,
    locked: bool,
}

pub type AccountMap = HashMap<u16, Account>;
pub type TransactionMap = HashMap<u32, Transaction>;

fn main() {
    // get the filename argument
    let arg: String = env::args().nth(1).expect("No csv file path given!");

    if let Err(err) = read_from_file(&arg) {
        println!("Could not read from file: {}", err);
        process::exit(1);
    }
}

fn read_from_file(path: &String) -> Result<(), Box<dyn Error>> {
    let mut accounts: AccountMap = HashMap::new();
    let mut transactions: TransactionMap = HashMap::new();

    // TODO: try tokio_codec::FramedRead
    let mut custom_reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .trim(Trim::All)
        .from_path(path)?;

    for result in custom_reader.deserialize() {
        let record: Transaction = result?;
        record.save(&mut transactions);
        let account_id = record.create_account_if_not_exists(&mut accounts);
        let account = accounts.entry(account_id);
        if record.r_type == "deposit" {
            account.and_modify(|this_account| this_account.deposit(record.amount));
        } else if record.r_type == "withdrawal" {
            account.and_modify(|this_account| this_account.withdraw(record.amount));
        } else if record.r_type == "dispute" {
            let referenced_tx_opt = transactions.get(&record.tx);
            match referenced_tx_opt {
                Some(referenced_tx) => {
                    account.and_modify(|this_account| this_account.dispute(referenced_tx.amount));
                }
                None => (), // ignore none case. TX does not exist
            }
        } else if record.r_type == "resolve" {
            let referenced_tx_opt = transactions.get(&record.tx);
            match referenced_tx_opt {
                Some(referenced_tx) => {
                    account.and_modify(|this_account| this_account.resolve(referenced_tx.amount));
                }
                None => (), // ignore none case. TX does not exist
            }
        } else if record.r_type == "chargeback" {
            let referenced_tx_opt = transactions.get(&record.tx);
            match referenced_tx_opt {
                Some(referenced_tx) => {
                    account
                        .and_modify(|this_account| this_account.chargeback(referenced_tx.amount));
                }
                None => (), // ignore none case. TX does not exist
            }
        }
    }
    csv_stdout(&accounts)?;
    Ok(())
}

pub fn four_precision_deserializer<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let given_f64: f64 = Option::deserialize(deserializer)?.unwrap_or(0.0);
    // could panic on unwrap
    let chopped_decimal = Decimal::from_f64(given_f64)
        .unwrap()
        .round_dp_with_strategy(4, RoundingStrategy::ToZero);
    let chopped_f64 = Decimal::to_f64(&chopped_decimal).unwrap_or(0.0);
    Ok(chopped_f64)
}

fn four_precision_serializer<S>(data: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // I should assume up to 4 precision. If given more than 4 precision, drop the extra.
    let chopped_decimal = Decimal::from_f64(*data)
        .unwrap()
        .round_dp_with_strategy(4, RoundingStrategy::ToZero);
    let chopped_f64 = Decimal::to_f64(&chopped_decimal).unwrap();
    serializer.serialize_f64(chopped_f64)
}

fn csv_stdout(accounts: &AccountMap) -> Result<(), Box<dyn Error>> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(io::stdout());
    for (_, account) in accounts.iter() {
        writer.serialize(account)?;
    }
    writer.flush()?;
    Ok(())
}

impl Transaction {
    fn save(&self, transactions: &mut TransactionMap) -> u32 {
        // only save on withdrawal or deposit
        if self.r_type == "withdrawal" || self.r_type == "deposit" {
            transactions.insert(
                self.tx,
                Transaction {
                    r_type: self.r_type.clone(),
                    amount: self.amount,
                    client: self.client,
                    tx: self.tx,
                },
            );
        }
        self.tx
    }
    fn create_account_if_not_exists(&self, accounts: &mut AccountMap) -> u16 {
        let account_opt = accounts.get(&self.client);
        match account_opt {
            Some(_) => self.client,
            None => {
                accounts.insert(
                    self.client,
                    Account {
                        available: 0.0,
                        client: self.client,
                        held: 0.0,
                        locked: false,
                        total: 0.0,
                    },
                );
                self.client
            }
        }
    }
}

impl Account {
    fn dispute(&mut self, amount: f64) {
        self.held = self.held + amount;
        self.available = self.total - self.held;
    }

    fn resolve(&mut self, amount: f64) {
        // ignore if not in dispute. aka nothing is held
        if self.held > 0.0 {
            self.held = self.held - amount;
            self.available = self.total - self.held;
        }
    }

    fn chargeback(&mut self, amount: f64) {
        // ignore if not in dispute. aka nothing is held
        if self.held > 0.0 {
            self.held = self.held - amount;
            self.total = self.total - amount;
            self.locked = true;
        }
    }

    fn deposit(&mut self, deposit_amount: f64) {
        // locked should prevent deposits and withdrawals
        if !self.locked {
            self.total = self.total + deposit_amount;
            self.available = self.available + deposit_amount;
        }
        
    }

    fn withdraw(&mut self, withdraw_amount: f64) {
        // check to make sure user does not overdraft
        // locked should prevent deposits and withdrawals
        if withdraw_amount < self.available && !self.locked {
            self.total = self.total - withdraw_amount;
            self.available = self.available - withdraw_amount;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_can_deposit() {
        let mut account = Account {
            available: 0.0,
            client: 1,
            held: 0.0,
            locked: false,
            total: 0.0
        };
        account.deposit(100.0);

        assert_eq!(account.available, 100.0);
        assert_eq!(account.total, 100.0)
    }
    #[test]
    fn account_cannot_overdraft() {
        let mut account = Account {
            available: 10.0,
            client: 1,
            held: 0.0,
            locked: false,
            total: 10.0
        };
        account.withdraw(9.0);

        // 1 left
        assert_eq!(account.available, 1.0);
        assert_eq!(account.total, 1.0);

        // try to take out 2.0
        account.withdraw(2.0);

        // unchanged
        assert_eq!(account.available, 1.0);
        assert_eq!(account.total, 1.0);

        account.dispute(0.5);

        // 0.5 available
        assert_eq!(account.held, 0.5);
        assert_eq!(account.available, 0.5);
        assert_eq!(account.total, 1.0);

        // try to take out 1.0
        account.withdraw(1.0);

        // unchanged
        assert_eq!(account.held, 0.5);
        assert_eq!(account.available, 0.5);
        assert_eq!(account.total, 1.0);
    }

    #[test]
    fn disputes_work() {
        let mut account = Account {
            available: 10.0,
            client: 1,
            held: 0.0,
            locked: false,
            total: 10.0
        };
        // let's pretend the tx had 5 in the amount
        account.dispute(5.0);
        // dispute locks 5 and reduces available
        assert_eq!(account.held, 5.0);
        assert_eq!(account.available, 5.0);
        // dispute locks another 3 and reduces available
        account.dispute(3.0);

        assert_eq!(account.held, 8.0);
        assert_eq!(account.available, 2.0);
        // resolve releases 3 from hold and increases available
        account.resolve(5.0);
        assert_eq!(account.held, 3.0);
        assert_eq!(account.available, 7.0);
        // chargeback removes 2 from total and reduces held. locks account.
        account.chargeback(2.0);
        assert_eq!(account.locked, true);
        assert_eq!(account.total, 8.0);
        // user tries to deposit on locked account
        account.deposit(1.0);
        // locked account prevents deposit
        assert_eq!(account.total, 8.0);
        // user tries to withdraw on locked account
        account.withdraw(1.0);
        // locked account prevents withdraw
        assert_eq!(account.total, 8.0);
    }

}