pub(crate) fn channel_client_builder(use_system_proxy: bool) -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    if use_system_proxy {
        builder
    } else {
        builder.no_proxy()
    }
}

pub(crate) fn channel_client(
    use_system_proxy: bool,
    timeout: std::time::Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    channel_client_builder(use_system_proxy)
        .timeout(timeout)
        .danger_accept_invalid_certs(true)
        .build()
}
