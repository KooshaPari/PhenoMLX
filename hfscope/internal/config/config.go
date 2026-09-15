// Package config loads server and HuggingFace configuration from environment
// variables (with sane defaults) so the binary can be tuned at deploy time.
package config

import (
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"

	"github.com/joho/godotenv"
)

// Config holds runtime settings for the HFScope server.
type Config struct {
	Addr           string        // listen address, e.g. ":8080"
	BaseURL        string        // external origin used when building share links
	HFBase         string        // HuggingFace API root
	UserAgent      string        // outbound UA for HF
	Token          string        // optional HF token (raises rate limits when set)
	TokenSource    string        // origin of the token for logging ("env:HF_TOKEN", "file:…", "none") — never the value
	DefaultLimit   int           // page size for the model list endpoint
	RequestTimeout time.Duration // per-request HTTP timeout
	CacheTTL       time.Duration // how long successful responses are cached
	CacheMaxAge    int           // HTTP cache-control max-age (seconds)
}

// Load builds a Config from environment variables. It never errors out —
// missing values fall back to defaults — so a fresh checkout boots without
// any setup. A .env file is loaded when present, but an explicit token file
// (HF_TOKEN_FILE, HF_TOKEN_PATH, or the default path) always wins.
func Load() Config {
	_ = godotenv.Load()

	// Pre-resolve: if a token file source is available and yields a valid
	// token, clear HF_TOKEN from the environment so godotenv's loading of
	// HF_TOKEN from .env doesn't shadow the user's deliberate file choice.
	if v, _ := readTokenFile(tokenFileEnvOrDefault()); v != "" {
		_ = os.Unsetenv("HF_TOKEN")
	}

	token, tokenSource := resolveToken()

	return Config{
		Addr:           getenv("HFSCOPE_ADDR", ":8080"),
		BaseURL:        getenv("HFSCOPE_BASE_URL", "http://localhost:8080"),
		HFBase:         getenv("HFSCOPE_HF_BASE", "https://huggingface.co"),
		UserAgent:      getenv("HFSCOPE_UA", "hfscope/0.1 (+https://huggingface.co)"),
		Token:          token,
		DefaultLimit:   getint("HFSCOPE_LIMIT", 24),
		RequestTimeout: time.Duration(getint("HFSCOPE_TIMEOUT_MS", 15000)) * time.Millisecond,
		CacheTTL:       time.Duration(getint("HFSCOPE_CACHE_TTL_S", 300)) * time.Second,
		CacheMaxAge:    getint("HFSCOPE_HTTP_MAX_AGE_S", 60),
		TokenSource:    tokenSource,
	}
}

// resolveToken looks for the HuggingFace token in this order:
//   1. HF_TOKEN_FILE / HF_TOKEN_PATH — explicit, secure, file-based (wins)
//   2. HF_TOKEN env var (plaintext, fast, good for containers/launchd)
//   3. ~/.config/hfscope/token (XDG-aware default location)
//   4. .env file (least secure — last resort, via godotenv)
//
// A file path provided via HF_TOKEN_FILE wins over a HF_TOKEN in env so that
// users can store the token in a 0600 file and still launch with both set.
//
// Returns the token (possibly empty) and a short label describing where it
// came from, safe to log. The token itself is never logged.
func resolveToken() (string, string) {
	// 1. Explicit file path (HF_TOKEN_FILE / HF_TOKEN_PATH) wins.
	{
		path := strings.TrimSpace(os.Getenv("HF_TOKEN_FILE"))
		if path == "" {
			path = strings.TrimSpace(os.Getenv("HF_TOKEN_PATH"))
		}
		if path != "" {
			if v, err := readTokenFile(path); err == nil && v != "" {
				return v, "file:" + filepath.Clean(path)
			} else if err != nil && !errors.Is(err, os.ErrNotExist) {
				fmt.Fprintf(os.Stderr, "hfscope: %v\n", err)
			}
		}
	}

	// 2. HF_TOKEN env var (or .env via godotenv upstream).
	if v := strings.TrimSpace(os.Getenv("HF_TOKEN")); v != "" {
		return v, "env:HF_TOKEN"
	}

	// 3. Default secure location: ~/.config/hfscope/token
	{
		path := DefaultTokenPath()
		if v, err := readTokenFile(path); err == nil && v != "" {
			return v, "file:" + filepath.Clean(path)
		} else if err != nil && !errors.Is(err, os.ErrNotExist) {
			fmt.Fprintf(os.Stderr, "hfscope: %v\n", err)
		}
	}

	return "", "none"
}

// readTokenFile loads a token from a single-line file. It enforces 0600
// permissions on Unix (configurable via HFSCOPE_TOKEN_FILE_FORCE_READ=1 to
// override on filesystems that don't support perms, e.g. some FUSE mounts).
// Returns os.ErrNotExist if the file is missing so the caller can fall back
// silently.
func readTokenFile(path string) (string, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	if runtime.GOOS != "windows" {
		if os.Getenv("HFSCOPE_TOKEN_FILE_FORCE_READ") != "1" {
			if info, statErr := os.Stat(path); statErr == nil {
				if perm := info.Mode().Perm(); perm&0o077 != 0 {
					return "", fmt.Errorf(
						"refusing to read token file %q with permissions %#o "+
							"(must be 0600 or 0700); chmod it or set "+
							"HFSCOPE_TOKEN_FILE_FORCE_READ=1 to bypass",
						path, perm)
				}
			}
		}
	}
	v := strings.TrimSpace(string(b))
	// Tolerate trailing CR (Windows line endings).
	v = strings.TrimRight(v, "\r")
	if i := strings.IndexByte(v, '\n'); i >= 0 {
		v = v[:i]
		v = strings.TrimSpace(v)
	}
	// Tolerate an accidental `KEY=VALUE` line (e.g. someone redirected a
	// whole .env line into the file). Strip the KEY= prefix if present.
	if eq := strings.IndexByte(v, '='); eq > 0 {
		head := strings.TrimSpace(v[:eq])
		if isIdent(head) {
			v = strings.TrimSpace(v[eq+1:])
		}
	}
	// Strip a single matching pair of quotes.
	if len(v) >= 2 {
		q := v[0]
		if (q == '"' || q == '\'') && v[len(v)-1] == q {
			v = v[1 : len(v)-1]
		}
	}
	return strings.TrimSpace(v), nil
}

// isIdent reports whether s is a non-empty env-var identifier (letters,
// digits, underscores; not starting with a digit).
func isIdent(s string) bool {
	if s == "" {
		return false
	}
	for i, r := range s {
		if r == '_' || ('A' <= r && r <= 'Z') || ('a' <= r && r <= 'z') {
			continue
		}
		if i > 0 && r >= '0' && r <= '9' {
			continue
		}
		return false
	}
	return true
}

// tokenFileEnvOrDefault returns the token file path: HF_TOKEN_FILE or
// HF_TOKEN_PATH if set, otherwise the default path.
func tokenFileEnvOrDefault() string {
	if p := strings.TrimSpace(os.Getenv("HF_TOKEN_FILE")); p != "" {
		return p
	}
	if p := strings.TrimSpace(os.Getenv("HF_TOKEN_PATH")); p != "" {
		return p
	}
	return DefaultTokenPath()
}

// DefaultTokenPath returns the XDG-respecting default token file path:
//   ~/.config/hfscope/token on Linux/macOS
//   %AppData%/hfscope/token on Windows
func DefaultTokenPath() string {
	if runtime.GOOS == "windows" {
		if appdata := os.Getenv("APPDATA"); appdata != "" {
			return filepath.Join(appdata, "hfscope", "token")
		}
	}
	if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
		return filepath.Join(xdg, "hfscope", "token")
	}
	if home, err := os.UserHomeDir(); err == nil {
		return filepath.Join(home, ".config", "hfscope", "token")
	}
	return filepath.Join(".config", "hfscope", "token")
}

// WriteDefaultTokenFile atomically writes the given token to the default
// path with 0600 permissions, creating parent dirs as needed. Useful for a
// setup helper script. Returns the resolved path.
func WriteDefaultTokenFile(token string) (string, error) {
	if token == "" {
		return "", errors.New("token is empty")
	}
	path := DefaultTokenPath()
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return "", err
	}
	tmp, err := os.CreateTemp(filepath.Dir(path), ".token-*")
	if err != nil {
		return "", err
	}
	tmpPath := tmp.Name()
	defer os.Remove(tmpPath) // no-op if we successfully renamed

	if _, err := tmp.WriteString(strings.TrimSpace(token) + "\n"); err != nil {
		tmp.Close()
		return "", err
	}
	if err := tmp.Chmod(0o600); err != nil {
		tmp.Close()
		return "", err
	}
	if err := tmp.Close(); err != nil {
		return "", err
	}
	if err := os.Rename(tmpPath, path); err != nil {
		return "", err
	}
	return path, nil
}

// IsProd returns true when the server is running in production mode.
func (c Config) IsProd() bool {
	return getenv("HFSCOPE_ENV", "dev") == "prod"
}

// HFEndpoint returns the absolute URL for a HuggingFace API path. It centralises
// base URL composition so callers don't sprinkle string concatenation around.
func (c Config) HFEndpoint(path string) string {
	return c.HFBase + path
}

func getenv(k, def string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return def
}

func getint(k string, def int) int {
	if v := os.Getenv(k); v != "" {
		if n, err := strconv.Atoi(v); err == nil {
			return n
		}
	}
	return def
}

// NormalizeBaseURL trims a trailing slash so we can concatenate paths safely.
func NormalizeBaseURL(raw string) string {
	u, err := url.Parse(raw)
	if err != nil || u.Scheme == "" {
		return raw
	}
	u.Path = ""
	return u.String()
}