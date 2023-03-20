# Tableau Monitoring `execd` for Telegraf  ![Rust](https://github.com/tfoldi/tableau-monitoring-execd/workflows/Rust/badge.svg)

Telegraf execd for getting Tableau Cluster status using TSM API and serverinfo.xml


## Usage

Command line parameters and environment variables:

```
    tableau-monitoring-execd [FLAGS] [OPTIONS]

FLAGS:
        --help            Prints help information
    -l, --passwordless    Use passwordless authentication [env: TME_TSM_PASSWORDLESS=]
    -V, --version         Prints version information

OPTIONS:
    -c, --checks <CHECKS>            Which method(s) to use for collecting status data [env: TME_CHECKS=] [default:
                                     all] [possible values: all, tsm, systeminfo]
    -s, --si-hostname <BASEURL>      Tableau Server's systeminfo web server base URL [env:
                                     TME_SI_HOSTNAME=] [default: https://localhost/]
    -h, --tsm-hostname <BASEURL>     Tableau Server TSM's base url [env: TME_TSM_HOSTNAME=]
                                     [default: https://localhost:8850/]
    -p, --tsm-password <PASSWORD>    PASSWORD for TSM Authentication [env: TME_TSM_PASSWORD=]
        --tsm-socket <tsm_socket>    TSM Socket to connect [env: TME_TSM_SOCKET=] [default:
                                     /var/run/tableau/tab-controller-login-8850]
    -u, --tsm-user <USERNAME>        Username for TSM Authentication [env: TME_TSM_USER=]
```

To use it from Telegraf, configure `[[input.execd]]` as:

```
# # Run executable as long-running input plugin
[[inputs.execd]]
   ## Program to run as daemon
   command = ["/path/to/tableau-monitoring-execd", "-p", "<password>", "-u", "<tsm user>", "-s", "https://tableauserver/"]
   signal = "STDIN"

   ## Delay before the process is restarted after an unexpected termination
   restart_delay = "10s"

   ## Data format to consume.
   ## Each data format has its own unique set of configuration options, read
   ## more about them here:
   ## https://github.com/influxdata/telegraf/blob/master/docs/DATA_FORMATS_INPUT.md
   data_format = "influx"
```

All configuration options are avaialbe as environement variables to avoid storing passwords as plain text in configuration files.

## License

BSD 2-Clause License, Tamas Foldi <tfoldi@starschema.com>
