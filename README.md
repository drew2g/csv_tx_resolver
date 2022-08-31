## Efficiency:

#### Hashmap as a database

When looking up transactions, we can either read the CSV every time or store the recent transactions in memory. Either way, this is acting as a database. In production, these hashmaps should be replaced with actual databases. 

#### Loading the csv in memory vs streaming with something like File::open and tokio_codec::FramedRead

I think it would be awesome to implement file streaming for large files or data from multiple sources. Since the database is in memory though, on high I/O it's likely to get too large - Especially since every single tx is stored for disputes. 


#### Enforcing four floating point

I know the test said I should 'assume' four floating points precision values are coming from the input. But what if they don't? Also: What if gas/network fees are a thing? Just to be safe I implemented a custom serializer/deserializer for accounts & transactions f64 values. It works but it's probably not the most efficient thing. It converts to a Decimal type which has helper functions for rounding down to a precision. Then it converts back to f64 for easy arithmetic and compatability. I could have done the string parse thing on '.' then taken the trailing decimal value and substringed up to the first 4 chars and reattached it. 

## Testing

Added some unit tests for testing the account manipulation funcitons `deposit`,`withdrawal`,`dispute`,`resolve` and `chargeback`. I did not add unit tests to test serialization/deserialization methods used to enforce the 4 floating point precision. Instead I included a csv that I ran through manually and checked the presision. I could have imported serde_test::{Token, assert_tokens} to test the serialization methods.

## Error Handling

Typically I use optionals where I can and try to handle the None cases. 

Serialization/Deserialization errors are typically the ones to be thrown. Overdraft, Account Locked, etc. errors are ignored so not to clutter the stdout. I could have had an enum for them and written them to standard error but 'cargo run -- transactions.csv > accounts.csv' would print standard error and mess up the csv.


## Maintainability

I could have spent some time to have a utils.rs file with the serialization/deserialization functions. The structs and their implementations could go in another file as well. 