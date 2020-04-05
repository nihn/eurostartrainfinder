# eurostartrainfinder
Rust CLI tool to get eurostar journeys matching supplied criteria, made to learn rust a bit.

## Usage
Just run help after `cargo build`, you will need eurostar API key which you can obtain inspecting their site in the browser:

```
mateuszm@mateuszm-mbp eurostarchecker % ./target/debug/eurostarchecker --help                                                                                                                                                            
eurostarchecker 0.1.0

USAGE:
    eurostarchecker [FLAGS] [OPTIONS] --api-key <api-key> --days <days> [ARGS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Verbose mode (-v, -vv, -vvv, etc.)

OPTIONS:
        --adults <adults>                 How many adults [default: 1]
    -a, --api-key <api-key>               Eurostar API key
    -d, --days <days>                     Number of days to stay (e.g. Friday - Sunday would be 3 days)
        --in-departure-after <HH:MM>      Only consider inbound trains departing after this time
        --in-departure-before <HH:MM>     Only consider inbound trains departing before this time
    -m, --max-price <max-price>           Max price per journey
        --out-departure-after <HH:MM>     Only consider outbound trains departing after this time
        --out-departure-before <HH:MM>    Only consider outbound trains departing before this time
    -s, --since <YYYY-MM-DD>              Since what date we should look [default: now]
        --sort-by <sort-by>               How results should be sorted [default: price]  [possible values: Price, Date]
    -u, --until <YYYY-MM-DD>              To what date we should look [default: +2 weeks]
    -w, --weekday <weekday>               Which days of the week should be considered as a start of a journey

ARGS:
    <from>    Start station [default: London]
    <to>      Finish station [default: Paris]
```
