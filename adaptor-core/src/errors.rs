use quick_error::quick_error;

quick_error! {
    #[derive(Debug)]
    pub enum AdaptorError {
        RusbError {
            from(rusb::Error)
        }
        SerdeError {
            from(postcard::Error)
        }
        NoDeviceError {}
        NoEndpointError {}
        ConnectionError {}
        SettingsError {}
        NotEnoughBytesSent {}
    }
}
