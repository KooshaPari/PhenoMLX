package server

import (
	"embed"
	"io/fs"
	"net/http"
	"os"
	"strings"

	"github.com/KooshaPari/hfscope/internal/config"
	"github.com/KooshaPari/hfscope/internal/views"
)

//go:embed static
var staticFS embed.FS

// PageData is the common payload threaded through every templ render.
// Aliased to the views package to avoid an import cycle with templ.
type PageData = views.PageData

// SortOption is one entry in the sort dropdown.
type SortOption = views.SortOption

// ErrorData is rendered when the server can't talk to HuggingFace.
type ErrorData = views.ErrorData

// staticHandler serves files from the embedded static/ directory under
// /static/. In dev (HFSCOPE_ENV != "prod") it falls back to the working
// tree so designers can edit CSS without re-running the build.
func staticHandler() http.Handler {
	sub, err := fs.Sub(staticFS, "static")
	if err != nil {
		return http.NotFoundHandler()
	}
	fileServer := http.FileServer(http.FS(sub))
	if !config.IsProdMode() {
		// Try the working tree first, fall back to embedded assets. This lets
		// `air` reload templates and CSS without a rebuild.
		live := http.FileServer(http.Dir("internal/server/static"))
		devFS := os.DirFS("internal/server/static")
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			r2 := r.Clone(r.Context())
			r2.URL.Path = strings.TrimPrefix(r.URL.Path, "/static/")
			if _, err := fs.Stat(devFS, r2.URL.Path); err == nil {
				live.ServeHTTP(w, r2)
				return
			}
			fileServer.ServeHTTP(w, r2)
		})
	}
	return fileServer
}