use clap::{crate_authors, crate_version, App, Arg};


fn main() {
    let mut app = App::new("tableau-monitoring-execd")
        .version(crate_version!())
        .author(crate_authors!())
        .about("telegraf execd for getting Tableau Cluster status using TSM API and serverinfo.xml")
        .arg(Arg::new("tsm_user")
            .short('u')
            .long("tsm-user")
            .value_name("USERNAME")
            .about("Username for TSM Authentication")
            .env("TME_TSM_USER")
            .takes_value(true)
        )
        .arg(Arg::new("tsm_password")
            .short('p')
            .long("tsm-password")
            .value_name("PASSWORD")
            .about("PASSWORD for TSM Authentication")
            .env("TME_TSM_PASSWORD")
            .takes_value(true)
        )
        .arg(Arg::new("tsm_hostname")
            .short('h')
            .long("tsm-hostname")
            .value_name("BASEURL")
            .about("Tableau Server TSM's base url")
            .env("TME_TSM_HOSTNAME")
            .default_value("https://localhost:8850/")
            .takes_value(true)
        )
        .arg(Arg::new("systeminfo_hostname")
            .short('s')
            .long("si-hostname")
            .value_name("BASEURL")
            .about("Tableau Server's systeminfo web server base URL")
            .env("TME_SI_HOSTNAME")
            .default_value("https://localhost/")
            .takes_value(true)
        )
        .arg(Arg::new("checks")
            .short('c')
            .long("checks")
            .value_name("CHECKS")
            .about("Username for TSM Authentication")
            .env("TME_CHECKS")
            .takes_value(true)
            .default_value("all")
            .possible_values(&["all", "tsm", "systeminfo"])
        );

    #[cfg(unix)]
        {
            app = app
                .arg(Arg::new("passwordless")
                    .short('l')
                    .long("passwordless")
                    .about("Use TSM passwordless authentication")
                    .env("TME_TSM_PASSWORDLESS")
                    .takes_value(false)
                )
                .arg(Arg::new("tsm_socket")
                    .long("tsm-socket")
                    .about("TSM Socket to connect")
                    .env("TME_TSM_SOCKET")
                    .default_value("/var/run/tableau/tab-controller-login-8850")
                    .takes_value(true));
        }

    tableau_monitoring_execd::run(&app.get_matches());
}
