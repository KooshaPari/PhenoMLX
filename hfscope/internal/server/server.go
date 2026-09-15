// Package server wires HTTP routes to the HuggingFace client and the templ
// templates. The server is intentionally framework-light: chi for routing,
// net/http for everything else.
package server

import (
	"context"
	"errors"
	"log/slog"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"

	"github.com/kooshapari/hfscope/internal/config"
	"github.com/kooshapari/hfscope/internal/hfapi"
	"github.com/kooshapari/hfscope/internal/views"
)

// Server is the HTTP application.
type Server struct {
	cfg   config.Config
	hf    *hfapi.Client
	log   *slog.Logger
	route chi.Router
}

// New constructs a Server. Templates are loaded eagerly so the first request
// doesn't pay the disk cost.
func New(cfg config.Config, hf *hfapi.Client, log *slog.Logger) *Server {
	s := &Server{cfg: cfg, hf: hf, log: log}
	s.route = s.routes()
	return s
}

// Handler returns the HTTP handler for use by http.Server.
func (s *Server) Handler() http.Handler { return s.route }

// routes wires middleware and routes.
func (s *Server) routes() chi.Router {
	r := chi.NewRouter()
	r.Use(middleware.RequestID)
	r.Use(middleware.RealIP)
	r.Use(middleware.Recoverer)
	r.Use(middleware.Compress(5))
	r.Use(s.securityHeaders)
	r.Use(s.requestLogger)

	// Static — served from disk in dev, embedded via embed.FS in prod.
	r.HandleFunc("/results/compare", s.handleCompare)
	r.HandleFunc("/feed", s.handleFeed)
	r.HandleFunc("/feed.rss", s.handleFeedRSS)
	r.Handle("/static/*", http.StripPrefix("/static/", staticHandler()))

	r.Get("/", s.handleHome)
	r.Get("/search", s.handleSearch)              // legacy: redirects to /results
	r.Get("/results", s.handleResults)            // htmx partial
	r.Get("/results/load-more", s.handleLoadMore) // htmx infinite scroll
	r.Get("/models/*", s.handleModel)             // model detail — wildcard so paths like /models/foo/bar work
	r.Get("/about", s.handleAbout)
	r.Get("/healthz", func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		_, _ = w.Write([]byte("ok"))
	})

	// Embed + JSON API surface (open CORS; see embed.go).
	r.Options("/embed/results", s.withCORS(func(w http.ResponseWriter, r *http.Request) {}))
	r.Options("/embed/model/*", s.withCORS(func(w http.ResponseWriter, r *http.Request) {}))
	r.Get("/embed/results", s.withCORS(s.handleEmbedResults))
	r.Get("/embed/model/*", s.withCORS(s.handleEmbedModel))
	r.Get("/api/models", s.withCORS(s.handleAPIModels))
	r.Get("/api/spaces", s.withCORS(s.handleAPIModels))
	r.Get("/api/datasets", s.withCORS(s.handleAPIModels))
	return r
}

// securityHeaders sets a conservative set of response headers.
func (s *Server) securityHeaders(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		h := w.Header()
		h.Set("X-Content-Type-Options", "nosniff")
		h.Set("Referrer-Policy", "strict-origin-when-cross-origin")
		h.Set("X-Frame-Options", "DENY")
		next.ServeHTTP(w, r)
	})
}

// requestLogger logs each request in a compact one-liner.
func (s *Server) requestLogger(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		ww := middleware.NewWrapResponseWriter(w, r.ProtoMajor)
		next.ServeHTTP(ww, r)
		s.log.Info("http",
			"method", r.Method,
			"path", r.URL.Path,
			"status", ww.Status(),
			"bytes", ww.BytesWritten(),
			"dur_ms", time.Since(start).Milliseconds(),
			"ua", r.UserAgent(),
			"hx", r.Header.Get("HX-Request"),
		)
	})
}

// parseSearch reads the SearchParams from an HTTP request.
func parseSearch(r *http.Request) hfapi.SearchParams {
	q := r.URL.Query()
	limit, _ := strconv.Atoi(q.Get("limit"))
	dir, _ := strconv.Atoi(q.Get("direction"))
	if dir == 0 {
		dir = -1
	}
	filters := q["filter"]
	if len(filters) == 0 {
		// Convenience: a single "filter" param can also be repeated
		if v := q.Get("filters"); v != "" {
			filters = strings.Split(v, ",")
		}
	}
	return hfapi.SearchParams{
		Kind:        hfapi.SearchKind(defaultStr(strings.ToLower(q.Get("type")), string(hfapi.KindModels))),
		Search:      strings.TrimSpace(q.Get("q")),
		Author:      strings.TrimSpace(q.Get("author")),
		Filter:      filters,
		PipelineTag: strings.TrimSpace(q.Get("task")),
		Library:     strings.TrimSpace(q.Get("library")),
		Language:    strings.TrimSpace(q.Get("language")),
		License:     strings.TrimSpace(q.Get("license")),
		Region:      strings.TrimSpace(q.Get("region")),
		Sort:        defaultStr(q.Get("sort"), "downloads"),
		Direction:   dir,
		Limit:       limit,
		Full:        hfapi.BoolFlag(q.Get("full")),
		Gated:       q.Get("gated"),
		Disabled:    q.Get("disabled"),
		Cursor:      q.Get("cursor"),
		WithCards:   hfapi.BoolFlag(q.Get("teaser")),
		Refine: hfapi.RefineParams{
			ParamsMin:     strings.TrimSpace(q.Get("params_min")),
			ParamsMax:     strings.TrimSpace(q.Get("params_max")),
			CreatedAfter:  strings.TrimSpace(q.Get("created_after")),
			CreatedBefore: strings.TrimSpace(q.Get("created_before")),
			MinDownloads:  strings.TrimSpace(q.Get("min_downloads")),
			MinLikes:      strings.TrimSpace(q.Get("min_likes")),
		},
	}
}

func defaultStr(v, def string) string {
	if v == "" {
		return def
	}
	return v
}

// handleHome renders the empty search form.
func (s *Server) handleHome(w http.ResponseWriter, r *http.Request) {
	s.cacheControl(w, 60)
	data := s.baseData(r, "Discover")
	renderHome(w, r, data)
}

// handleSearch is the legacy filter-form-only page. It now redirects to
// /results so the sidebar is always available; deep links with query
// params are preserved.
func (s *Server) handleSearch(w http.ResponseWriter, r *http.Request) {
	target := "/results"
	if r.URL.RawQuery != "" {
		target += "?" + r.URL.RawQuery
	}
	http.Redirect(w, r, target, http.StatusFound)
}

// handleResults performs a search and returns the full page (initial load)
// or just the result list (HTMX partial). HTMX is detected via the
// HX-Request header that the library sends. With no filters at all, this
// becomes the browse mode: top-downloaded models with the sidebar live.
func (s *Server) handleResults(w http.ResponseWriter, r *http.Request) {
	params := parseSearch(r)
	if params.Limit <= 0 {
		params.Limit = s.cfg.DefaultLimit
	}
	// Browse mode always sorts by downloads descending when nothing else is set.
	if params.Sort == "" {
		params.Sort = "downloads"
	}
	if params.Direction == 0 {
		params.Direction = -1
	}
	result, err := s.hf.ListModels(r.Context(), params)
	if err != nil {
		s.renderError(w, r, err, http.StatusBadGateway)
		return
	}
	data := s.baseData(r, "Browse")
	data.Params = params
	data.Result = result
	data.Sorts = sortOptions()
	data.Tags = s.fetchTags(r.Context())
	// Card teasers are opt-in via ?teaser=1 so we don't hammer HF for users
	// who never asked. We hydrate only the first 6 cards on the page.
	if params.WithCards && len(result.Models) > 0 {
		data.Teasers = s.hydrateTeasers(r.Context(), result.Models)
	}
	if isHTMX(r) {
		renderResultsPartial(w, r, data)
		return
	}
	s.cacheControl(w, s.cfg.CacheMaxAge)
	renderResults(w, r, data)
}

// handleLoadMore returns the next page for HTMX infinite scroll. It expects
// a "cursor" query parameter and returns the new models fragment with
// HX-Trigger headers so the client can swap state.
func (s *Server) handleLoadMore(w http.ResponseWriter, r *http.Request) {
	params := parseSearch(r)
	if params.Limit <= 0 {
		params.Limit = s.cfg.DefaultLimit
	}
	if params.Sort == "" {
		params.Sort = "downloads"
	}
	if params.Direction == 0 {
		params.Direction = -1
	}
	if params.Cursor == "" {
		http.Error(w, "cursor required", http.StatusBadRequest)
		return
	}
	result, err := s.hf.ListModels(r.Context(), params)
	if err != nil {
		s.renderError(w, r, err, http.StatusBadGateway)
		return
	}
	data := s.baseData(r, "Browse")
	data.Params = params
	data.Result = result
	data.Sorts = sortOptions()
	data.Tags = s.fetchTags(r.Context())
	w.Header().Set("HX-Trigger", "results:appended")
	renderResultsPartial(w, r, data)
}

// handleModel renders a single model's detail page. We fetch the lightweight
// model first (fast), then attempt a lazy hydrate of /api/models/{id} for the
// rich description block (cardData, description, transformers_info). The rich
// fetch is non-fatal: if it fails we still serve the page with whatever we
// have. The TTL cache keeps repeat views cheap.
func (s *Server) handleModel(w http.ResponseWriter, r *http.Request) {
	repoID := chi.URLParam(r, "*")
	if repoID == "" {
		http.Error(w, "missing repo id", http.StatusBadRequest)
		return
	}
	m, err := s.hf.GetModel(r.Context(), repoID)
	if err != nil {
		if errors.Is(err, hfapi.ErrNotFound) {
			http.Error(w, "model not found", http.StatusNotFound)
			return
		}
		s.renderError(w, r, err, http.StatusBadGateway)
		return
	}
	data := s.baseData(r, m.ID)
	data.Model = m
	data.Summary = hfapi.Summarize(m)
	if md, derr := s.hf.GetModelDetail(r.Context(), repoID); derr == nil {
		data.ModelDetail = md
	} else {
		data.DetailErr = derr.Error()
		s.log.Warn("detail hydrate failed", "repo", repoID, "err", derr)
	}
	s.cacheControl(w, 60)
	renderModel(w, r, data)
}

// hydrateTeasers fetches description teasers for the first N models in the
// page, with concurrency capped at 3 in-flight calls. A buffered-channel
// semaphore is used (no external library) so a slow HF lookup can't pin
// every goroutine. Each call also has its own per-fetch timeout so one
// stuck response doesn't block the rest. Failures on individual lookups
// are silent (we just don't render a teaser for that card). The TTL cache
// means repeats across requests are free.
func (s *Server) hydrateTeasers(parent context.Context, models []hfapi.Model) map[string]string {
	const (
		limit        = 6
		maxInFlight  = 3
		fetchTimeout = 4 * time.Second
	)
	if len(models) > limit {
		models = models[:limit]
	}
	type result struct {
		id     string
		teaser string
	}
	out := make([]result, len(models))
	// Buffered channel acts as a counting semaphore: each goroutine must
	// acquire a slot before issuing the request and releases it on return.
	sem := make(chan struct{}, maxInFlight)
	var wg sync.WaitGroup
	for i, m := range models {
		i, m := i, m
		wg.Add(1)
		go func() {
			defer wg.Done()
			// Bound the wait so a saturated semaphore can't stall us past
			// the request's overall context deadline.
			select {
			case sem <- struct{}{}:
			case <-parent.Done():
				return
			}
			defer func() { <-sem }()

			ctx, cancel := context.WithTimeout(parent, fetchTimeout)
			defer cancel()
			md, err := s.hf.GetModelDetail(ctx, m.ID)
			if err != nil {
				return
			}
			out[i] = result{id: m.ID, teaser: hfapi.DescriptionTeaser(md)}
		}()
	}
	wg.Wait()
	teasers := map[string]string{}
	for _, r := range out {
		if r.teaser != "" {
			teasers[r.id] = r.teaser
		}
	}
	return teasers
}

// handleAbout renders the about page.
func (s *Server) handleAbout(w http.ResponseWriter, r *http.Request) {
	s.cacheControl(w, 600)
	data := s.baseData(r, "About")
	renderAbout(w, r, data)
}

// baseData populates the PageData struct shared by every page render.
func (s *Server) baseData(r *http.Request, title string) PageData {
	return PageData{
		Title:       title,
		BaseURL:     config.NormalizeBaseURL(s.cfg.BaseURL),
		HFBase:      s.cfg.HFBase,
		IsProd:      s.cfg.IsProd(),
		Path:        r.URL.Path,
		Params:      parseSearch(r),
		Sorts:       sortOptions(),
		GeneratedAt: time.Now().UTC(),
	}
}

// fetchTags returns the cached tag taxonomy or, on first miss, fetches it.
// Errors are non-fatal; the UI simply omits the dependent controls.
func (s *Server) fetchTags(ctx context.Context) hfapi.TagGroups {
	g, err := s.hf.Tags(ctx)
	if err != nil {
		s.log.Warn("tags fetch failed", "err", err)
		return nil
	}
	return g
}

// cacheControl sets a Cache-Control header when running in production.
// During development we set no-cache so template changes are reflected.
func (s *Server) cacheControl(w http.ResponseWriter, maxAge int) {
	if s.cfg.IsProd() && maxAge > 0 {
		w.Header().Set("Cache-Control", "public, max-age="+strconv.Itoa(maxAge))
		return
	}
	w.Header().Set("Cache-Control", "no-cache")
}

// renderError writes a templ error page. When the request is from HTMX it
// sends a 200 with an OOB swap so the partial state stays consistent.
func (s *Server) renderError(w http.ResponseWriter, r *http.Request, err error, code int) {
	s.log.Error("upstream error", "err", err, "path", r.URL.Path)
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(code)
	renderError(w, r, ErrorData{Title: "Upstream error", Message: err.Error()})
}

// isHTMX returns true when the request was made by HTMX.
func isHTMX(r *http.Request) bool {
	return r.Header.Get("HX-Request") == "true"
}

// sortOptions returns the (label, value) pairs offered in the sort menu.
func sortOptions() []SortOption {
	return []SortOption{
		{Label: "Most downloads", Value: "downloads", Direction: -1},
		{Label: "Most likes", Value: "likes", Direction: -1},
		{Label: "Trending", Value: "trendingScore", Direction: -1},
		{Label: "Recently modified", Value: "lastModified", Direction: -1},
		{Label: "Oldest modified", Value: "lastModified", Direction: 1},
	}
}

// shareLinkFor returns an absolute URL for the given route.
func (s *Server) shareLinkFor(r *http.Request, path string) string {
	u, err := url.Parse(s.cfg.BaseURL)
	if err != nil || u.Scheme == "" {
		return path
	}
	u.Path = path
	return u.String()
}

// parseCompareIDs extracts up to 4 repo ids from the "ids" query parameter,
// splitting on commas, trimming whitespace, and de-duplicating.
func parseCompareIDs(raw string) []string {
	if raw == "" {
		return nil
	}
	seen := map[string]bool{}
	out := []string{}
	for _, p := range strings.Split(raw, ",") {
		id := strings.TrimSpace(p)
		if id == "" || seen[id] {
			continue
		}
		seen[id] = true
		out = append(out, id)
		if len(out) == 4 {
			break
		}
	}
	return out
}

// fetchCompareRows concurrently fetches GetModelDetail for each id, bounded
// to 4 in-flight at once with an 8-second per-call timeout. Graceful
// degradation: partial rows render with whatever data arrived, missing ones
// show an error placeholder in the column.
func (s *Server) fetchCompareRows(ctx context.Context, ids []string) []views.CompareColumn {
	out := make([]views.CompareColumn, len(ids))
	var wg sync.WaitGroup
	sem := make(chan struct{}, 4)
	for i, id := range ids {
		wg.Add(1)
		go func(i int, id string) {
			defer wg.Done()
			select {
			case sem <- struct{}{}:
			case <-ctx.Done():
				out[i] = views.CompareColumn{ID: id, Error: "context cancelled"}
				return
			}
			defer func() { <-sem }()
			cctx, cancel := context.WithTimeout(ctx, 8*time.Second)
			defer cancel()
			d, err := s.hf.GetModelDetail(cctx, id)
			col := views.CompareColumn{ID: id}
			if err != nil {
				col.Error = err.Error()
				out[i] = col
				return
			}
			// Build a lean Model for column display (avoid passing the whole detail).
			col.Model = hfapi.Model{
				ID:          d.ID,
				Author:      authorOf(id),
				Downloads:   d.Downloads,
				Likes:       d.Likes,
				PipelineTag: d.PipelineTag,
				Tags:        d.Tags,
			}
			col.Summary = hfapi.Summarize(col.Model)
			col.Detail = d
			out[i] = col
		}(i, id)
	}
	wg.Wait()
	return out
}

// authorOf returns the author portion of an HF repo id ("a/b" -> "a").
func authorOf(id string) string {
	if i := strings.Index(id, "/"); i > 0 {
		return id[:i]
	}
	return ""
}

// handleCompare serves /results/compare?ids=a,b,c — a side-by-side compare
// page of up to 4 models. Renders columns with shared metrics and per-model
// file counts. If no ids are provided, redirects to /results.
func (s *Server) handleCompare(w http.ResponseWriter, r *http.Request) {
	data := s.baseData(r, "Compare models — HFScope")
	data.CompareIDs = parseCompareIDs(r.URL.Query().Get("ids"))
	if len(data.CompareIDs) == 0 {
		http.Redirect(w, r, "/results", http.StatusFound)
		return
	}
	data.CompareRows = s.fetchCompareRows(r.Context(), data.CompareIDs)
	renderCompare(w, r, data)
}

// handleFeed serves the HTML landing page for the RSS feed at /feed.
// Lists available query params and shows a "Copy URL" link.
func (s *Server) handleFeed(w http.ResponseWriter, r *http.Request) {
	data := s.baseData(r, "RSS feed — HFScope")
	q := r.URL.Query().Encode()
	if q != "" {
		q = "?" + q
	}
	data.FeedHref = "/feed.rss" + q
	renderFeed(w, r, data)
}

// handleFeedRSS serves RSS 2.0 XML for the top-N models matching the current
// filter state. Same params as /results. Cap at 50 items to keep the feed
// reasonable.
func (s *Server) handleFeedRSS(w http.ResponseWriter, r *http.Request) {
	params := parseSearch(r)
	if params.Limit <= 0 || params.Limit > 50 {
		params.Limit = 24
	}
	res, err := s.hf.ListModels(r.Context(), params)
	if err != nil {
		s.log.Error("feed: list models", "err", err)
		http.Error(w, "upstream error", http.StatusBadGateway)
		return
	}
	w.Header().Set("Content-Type", "application/rss+xml; charset=utf-8")
	w.Header().Set("Cache-Control", "public, max-age=300")
	w.Write([]byte(renderFeedXML(r, params, res.Models, s.cfg.HFBase)))
}
