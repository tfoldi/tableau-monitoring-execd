use ureq::{Agent, AgentBuilder};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};


fn get_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn get_system_info_xml(agent: &Agent, url: &String) -> Result<String,ureq::Error> {
    let xml_server_info = agent.get(url)
        .call()?
        .into_string()?;

    Ok(xml_server_info)
}

fn parse_system_info(xml: &String) -> Result<(),roxmltree::Error>{
    let doc = roxmltree::Document::parse(&*xml)?;

    for node in doc.descendants() {
        let tag_name = node.tag_name().name();

        if tag_name == "systeminfo" || tag_name == "machines" || tag_name == "machine"  {
            continue;
        } else if tag_name == "service" {
            match node.attribute("status") {
                None => eprintln!("Cannot get overall cluster status, attribute is missing"),
                Some(status) =>
                    println!("tableau_systeminfo status={} {}", status, get_epoch_ms())

            }
        }

        println!("{}", tag_name);
    };

   Ok(())
}

fn check_system_info(agent: &Agent, url: &String) {

    match get_system_info_xml(&agent, url) {
        Err(err) => eprintln!("Cannot fetch systeminfo.xml: {}", err.to_string()),
        Ok(xml) => {
            if let Err(err) = parse_system_info(&xml) {
                eprintln!("Cannot parse systeminfo.xml: {}", err.to_string());
            }
        }
    };
}

fn main() {
    let agent: Agent = AgentBuilder::new()
        .timeout_read(Duration::from_secs(5))
        .timeout_write(Duration::from_secs(5))
        .build();

    check_system_info(&agent, &"https://insights.starschema.net/admin/systeminfo.xml".to_string());
}
