mod support;

mod version {
    use super::support::Sandbox;

    #[test]
    fn prints_crate_version() {
        // Arrange
        let sandbox = Sandbox::new();

        // Act
        let run = sandbox.wrkt(&["--version"]);

        // Assert
        assert_eq!(run.success().stdout(), "wrkt 0.1.0\n");
    }
}
