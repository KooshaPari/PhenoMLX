package config

// IsProdMode is a package-level helper so other packages can detect mode
// without depending on a Config value being threaded through.
func IsProdMode() bool { return getenv("HFSCOPE_ENV", "dev") == "prod" }