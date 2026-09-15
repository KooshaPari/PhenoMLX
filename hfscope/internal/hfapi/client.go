// Package hfapi is a thin, opinionated client around the public HuggingFace
// Hub REST API. It exposes typed helpers for the endpoints HFScope needs:
// listing models, fetching a single model, and pulling the canonical tag
// taxonomy. The client handles authentication, timeouts, JSON decoding, and
// a caller-provided cache so the UI feels instant.
package hfapi

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/kooshapari/hfscope/internal/cache"
)

// Client talks to the HuggingFace Hub.
type Client struct {
	http    *http.Client
	base    string
	token   string
	ua      string
	cache   *cache.TTL
	maxKeys int // defence against enormous fanouts (unused today)
}

// Options configures a new Client.
type Options struct {
	BaseURL       string        // e.g. "https://huggingface.co"
	Token         string        // optional HF user token
	UserAgent     string        // outbound UA
	Timeout       time.Duration // per-request timeout
	Cache         *cache.TTL    // optional response cache
}

// New returns a configured Client. A zero Timeout falls back to 15s.
func New(o Options) *Client {
	if o.Timeout <= 0 {
		o.Timeout = 15 * time.Second
	}
	if o.UserAgent == "" {
		o.UserAgent = "hfscope/0.1"
	}
	return &Client{
		http:  &http.Client{Timeout: o.Timeout},
		base:  strings.TrimRight(o.BaseURL, "/"),
		token: o.Token,
		ua:    o.UserAgent,
		cache: o.Cache,
	}
}

// WithToken returns a copy of the Client that authenticates every request
// with the given token. The receiver is not mutated, so per-request tokened
// clients (e.g. from an Authorization header) are safe to derive.
func (c *Client) WithToken(token string) *Client {
	cp := *c
	cp.token = token
	return &cp
}

// Model is the subset of model fields that HFScope renders. We keep this
// minimal so the UI is fast and we can evolve independently of upstream
// schema changes.
type Model struct {
	ID            string         `json:"id"`
	ModelID       string         `json:"modelId"`
	PipelineTag   string         `json:"pipeline_tag"`
	LibraryName   string         `json:"library_name"`
	Tags          []string       `json:"tags"`
	Likes         int            `json:"likes"`
	Downloads     int            `json:"downloads"`
	TrendingScore int            `json:"trendingScore"`
	Private       bool           `json:"private"`
	Disabled      bool           `json:"disabled"`
	Gated         any            `json:"gated"` // false (bool) | "auto" | "manual" | true
	CreatedAt     string         `json:"createdAt"`
	LastModified  string         `json:"lastModified"`
	Author        string         `json:"author"`
	SHA           string         `json:"sha"`
	Safetensors   map[string]any `json:"safetensors"` // optional — only on rich / full=true responses
}

// FileEntry is one sibling in a model repo (a file in the tree).
// HF returns `{ "rfilename": "model.safetensors" }` for each; we
// also keep the resolved Size when the cache populates it.
type FileEntry struct {
	RFilename string `json:"rfilename"`
	Size      int64  `json:"size,omitempty"`
}

// FileCategory groups FileEntry by extension. Used by the file-grid
// and config-block UI to bucket siblings (e.g. SafeTensors, GGUF, PyTorch,
// JSON, Tokenizer, Markdown, Config, Other).
type FileCategory string

const (
	CategorySafetensors FileCategory = "safetensors"
	CategoryGGUF        FileCategory = "gguf"
	CategoryBin         FileCategory = "bin"
	CategoryPT          FileCategory = "pt"
	CategoryJSON        FileCategory = "json"
	CategoryTokenizer   FileCategory = "tokenizer"
	CategoryMarkdown    FileCategory = "md"
	CategoryConfig      FileCategory = "config"
	CategoryOther       FileCategory = "other"
)

// Aliases so filegrid.templ can use the FileCat* prefix it expects.
const (
	FileCatSafetensors = CategorySafetensors
	FileCatGGUF        = CategoryGGUF
	FileCatBin         = CategoryBin
	FileCatPT          = CategoryPT
	FileCatJSON        = CategoryJSON
	FileCatTokenizer   = CategoryTokenizer
	FileCatMarkdown    = CategoryMarkdown
	FileCatConfig      = CategoryConfig
	FileCatOther       = CategoryOther
)

// AllCategories returns every defined FileCategory in canonical display
// order, suitable for iterating in the UI.
func AllCategories() []FileCategory {
	return []FileCategory{
		CategorySafetensors,
		CategoryGGUF,
		CategoryBin,
		CategoryPT,
		CategoryConfig,
		CategoryTokenizer,
		CategoryJSON,
		CategoryMarkdown,
		CategoryOther,
	}
}

// ID returns the canonical string form of the category.

func (c FileCategory) ID() string { return string(c) }

// Label returns the human-readable label used in the file-grid UI.
func (c FileCategory) Label() string {
	switch c {
	case CategorySafetensors:
		return "SafeTensors"
	case CategoryGGUF:
		return "GGUF"
	case CategoryBin:
		return "Bin"
	case CategoryPT:
		return "PyTorch"
	case CategoryJSON:
		return "JSON"
	case CategoryTokenizer:
		return "Tokenizer"
	case CategoryMarkdown:
		return "Markdown"
	case CategoryConfig:
		return "Config"
	case CategoryOther:
		return "Other"
	}
	return string(c)
}

// Order returns a small integer used to sort category chips in a consistent
// visual order: SafeTensors → GGUF → PyTorch → Bin → Config → JSON →
// Tokenizer → Markdown → Other.
func (c FileCategory) Order() int {
	switch c {
	case CategorySafetensors:
		return 0
	case CategoryGGUF:
		return 1
	case CategoryPT:
		return 2
	case CategoryBin:
		return 3
	case CategoryConfig:
		return 4
	case CategoryJSON:
		return 5
	case CategoryTokenizer:
		return 6
	case CategoryMarkdown:
		return 7
	case CategoryOther:
		return 8
	}
	return 99
}

// OrderKey returns a sortable string for use with sort.Slice.
func (c FileCategory) OrderKey() string { return fmt.Sprintf("%02d-%s", c.Order(), c) }

// ModelDetail is the rich shape returned by GET /api/models/{id}. Only the
// single-model endpoint exposes cardData, description, model-index, and the
// transformers_info block. We hydrate it lazily when the user opens a detail
// page — never on the list endpoint (which would explode to 24× the cost).
type ModelDetail struct {
	Model
	CardData     map[string]any `json:"cardData"`
	Transformers map[string]any `json:"transformers_info"`
	Description  string         `json:"description,omitempty"`
	Spaces       []string       `json:"spaces,omitempty"`
	WidgetData   any            `json:"widgetData,omitempty"`
	ModelIndex   map[string]any `json:"model-index"`
	UsedStorage  int64          `json:"usedStorage"`
	Config       map[string]any `json:"config"`
	Siblings     []FileEntry    `json:"siblings"`
}

// SearchKind picks which HuggingFace hub collection to query.
// "models" (default) hits /api/models, "spaces" hits /api/spaces,
// "datasets" hits /api/datasets. Other values fall back to "models".
type SearchKind string

const (
	KindModels   SearchKind = "models"
	KindSpaces   SearchKind = "spaces"
	KindDatasets SearchKind = "datasets"
)

// SearchParams is the public query surface for the models endpoint.
type SearchParams struct {
	Kind        SearchKind // "models" (default), "spaces", or "datasets"
	Search      string
	Author      string
	Filter      []string // multi-value 'filter' (e.g. ["text-generation","en"])
	PipelineTag string
	Library     string
	Language    string
	License     string
	Sort        string // downloads | likes | lastModified | trendingScore
	Direction   int    // -1 or 1
	Limit       int
	Full        bool
	Gated       string // "", "true", "false" (models only)
	Disabled    string // "", "true", "false" (models only)
	Region      string // "", "us", "eu", "asia", "cn"
	Cursor      string
	WithCards   bool   // when true, hydrate each result's description teaser

	// Refine holds the client-side filter thresholds carried in the URL.
	// The upstream /api/models endpoint does not honor these, but the URL
	// is the single source of truth so deep-links reproduce the visible state
	// via hfscope.js's filter engine.
	Refine RefineParams
}

// RefineParams are the optional, client-side filter thresholds.
type RefineParams struct {
	ParamsMin    string // params slider low (raw param count as a string)
	ParamsMax    string // params slider high
	CreatedAfter string // ISO date (YYYY-MM-DD) or empty
	CreatedBefore string
	MinDownloads string
	MinLikes     string
}

// ListResult is the paged response from the models list endpoint.
type ListResult struct {
	Models   []Model
	NextLink string // raw Link header (rel=next) for cursor pagination
	Total    int
}

// encode builds the URL-encoded query string for a SearchParams. Exposed so
// callers can compute cache keys or build share links.
func (p SearchParams) encode() string {
	q := url.Values{}
	if p.Search != "" {
		q.Set("search", p.Search)
	}
	if p.Author != "" {
		q.Set("author", p.Author)
	}
	for _, f := range p.Filter {
		q.Add("filter", f)
	}
	if p.PipelineTag != "" {
		q.Set("pipeline_tag", p.PipelineTag)
	}
	if p.Library != "" {
		q.Set("library", p.Library)
	}
	if p.Language != "" {
		q.Set("language", p.Language)
	}
	if p.License != "" {
		q.Set("license", p.License)
	}
	if p.Sort != "" {
		q.Set("sort", p.Sort)
	}
	if p.Direction != 0 {
		q.Set("direction", strconv.Itoa(p.Direction))
	}
	if p.Limit > 0 {
		q.Set("limit", strconv.Itoa(p.Limit))
	}
	if p.Full {
		q.Set("full", "true")
	}
	if p.Gated != "" {
		q.Set("gated", p.Gated)
	}
	if p.Disabled != "" {
		q.Set("disabled", p.Disabled)
	}
	if p.Region != "" {
		q.Add("filter", "region:"+p.Region)
	}
	if p.Cursor != "" {
		q.Set("cursor", p.Cursor)
	}
	return q.Encode()
}

// Key produces a stable cache key for a SearchParams.
func (p SearchParams) Key() string { return "models:" + p.encode() }

// ErrNotFound is returned when the upstream returns 404.
var ErrNotFound = errors.New("huggingface: not found")

// do performs a cached HTTP GET against the configured base URL.
func (c *Client) do(ctx context.Context, path, key string) ([]byte, http.Header, int, error) {
	if c.cache != nil {
		if e, ok := c.cache.Get(key); ok {
			h := http.Header{}
			for k, v := range e.Header {
				h.Set(k, v)
			}
			return e.Body, h, e.StatusCode, nil
		}
	}
	u := c.base + path
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, u, nil)
	if err != nil {
		return nil, nil, 0, err
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("User-Agent", c.ua)
	if c.token != "" {
		req.Header.Set("Authorization", "Bearer "+c.token)
	}
	resp, err := c.http.Do(req)
	if err != nil {
		return nil, nil, 0, err
	}
	defer resp.Body.Close()
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, nil, resp.StatusCode, err
	}
	if c.cache != nil && resp.StatusCode == http.StatusOK {
		hdrs := map[string]string{}
		for k, v := range resp.Header {
			if len(v) > 0 {
				hdrs[k] = v[0]
			}
		}
		c.cache.Set(key, body, hdrs, resp.StatusCode)
	}
	return body, resp.Header, resp.StatusCode, nil
}

// ListModels hits GET /api/models with the given SearchParams.
func (c *Client) ListModels(ctx context.Context, p SearchParams) (ListResult, error) {
	q := p.encode()
	path := "/api/models"
	if q != "" {
		path += "?" + q
	}
	body, hdr, status, err := c.do(ctx, path, p.Key())
	if err != nil {
		return ListResult{}, err
	}
	if status == http.StatusNotFound {
		return ListResult{}, ErrNotFound
	}
	if status != http.StatusOK {
		return ListResult{}, fmt.Errorf("huggingface: status %d: %s", status, truncate(body, 200))
	}
	var models []Model
	if err := json.Unmarshal(body, &models); err != nil {
		return ListResult{}, fmt.Errorf("huggingface: decode: %w", err)
	}
	next := ""
	if link := hdr.Get("Link"); link != "" {
		next = parseNextLink(link)
	}
	return ListResult{Models: models, NextLink: next}, nil
}

// GetModel hits GET /api/models/{repo_id}.
func (c *Client) GetModel(ctx context.Context, id string) (Model, error) {
	if id == "" {
		return Model{}, errors.New("huggingface: empty model id")
	}
	key := "model:" + id
	body, _, status, err := c.do(ctx, "/api/models/"+id, key)
	if err != nil {
		return Model{}, err
	}
	if status == http.StatusNotFound {
		return Model{}, ErrNotFound
	}
	if status != http.StatusOK {
		return Model{}, fmt.Errorf("huggingface: status %d: %s", status, truncate(body, 200))
	}
	var m Model
	if err := json.Unmarshal(body, &m); err != nil {
		return Model{}, fmt.Errorf("huggingface: decode: %w", err)
	}
	return m, nil
}

// GetModelDetail hits GET /api/models/{repo_id} and decodes the rich shape
// (cardData, description, transformers_info, model-index, etc.). The result
// feeds the description block on the model detail page.
func (c *Client) GetModelDetail(ctx context.Context, id string) (ModelDetail, error) {
	if id == "" {
		return ModelDetail{}, errors.New("huggingface: empty model id")
	}
	key := "model-detail:" + id
	body, _, status, err := c.do(ctx, "/api/models/"+id, key)
	if err != nil {
		return ModelDetail{}, err
	}
	if status == http.StatusNotFound {
		return ModelDetail{}, ErrNotFound
	}
	if status != http.StatusOK {
		return ModelDetail{}, fmt.Errorf("huggingface: status %d: %s", status, truncate(body, 200))
	}
	var md ModelDetail
	if err := json.Unmarshal(body, &md); err != nil {
		return ModelDetail{}, fmt.Errorf("huggingface: decode: %w", err)
	}
	return md, nil
}

// Tag is a single taxonomy entry returned by /api/models-tags-by-type.
type Tag struct {
	ID    string `json:"id"`
	Label string `json:"label"`
	Type  string `json:"type"`
}

// TagGroups is the grouped response from /api/models-tags-by-type.
type TagGroups map[string][]Tag

// Tags fetches and caches the tag taxonomy. The response is small and
// nearly static, so it lives in the same TTL cache.
func (c *Client) Tags(ctx context.Context) (TagGroups, error) {
	return c.modelTags(ctx, "/api/models-tags-by-type", "tags:models")
}

// SpaceTags fetches the tag taxonomy for Spaces.
func (c *Client) SpaceTags(ctx context.Context) (TagGroups, error) {
	return c.modelTags(ctx, "/api/spaces-tags-by-type", "tags:spaces")
}

// DatasetTags fetches the tag taxonomy for Datasets.
func (c *Client) DatasetTags(ctx context.Context) (TagGroups, error) {
	return c.modelTags(ctx, "/api/datasets-tags-by-type", "tags:datasets")
}

func (c *Client) modelTags(ctx context.Context, path, key string) (TagGroups, error) {
	body, _, status, err := c.do(ctx, path, key)
	if err != nil {
		return nil, err
	}
	if status != http.StatusOK {
		return nil, fmt.Errorf("huggingface: tags %s status %d: %s", path, status, truncate(body, 200))
	}
	var g TagGroups
	if err := json.Unmarshal(body, &g); err != nil {
		return nil, err
	}
	return g, nil
}

// ListSpaces fetches a page of Spaces matching params. Mirrors ListModels but
// hits /api/spaces. Use Limit ≤ 50 on the wire so the page survives a slow
// upstream. The same cursor Link header drives pagination.
func (c *Client) ListSpaces(ctx context.Context, p SearchParams) (ListResult, error) {
	return c.listByPath(ctx, p, "/api/spaces")
}

// ListDatasets fetches a page of Datasets matching params. Mirrors ListModels but
// hits /api/datasets.
func (c *Client) ListDatasets(ctx context.Context, p SearchParams) (ListResult, error) {
	return c.listByPath(ctx, p, "/api/datasets")
}

func (c *Client) listByPath(ctx context.Context, p SearchParams, path string) (ListResult, error) {
	q := url.Values{}
	if p.Search != "" {
		q.Set("search", p.Search)
	}
	if p.Author != "" {
		q.Set("author", p.Author)
	}
	if p.Limit > 0 {
		q.Set("limit", strconv.Itoa(p.Limit))
	}
	if p.Cursor != "" {
		q.Set("cursor", p.Cursor)
	}
	for _, t := range p.Filter {
		q.Add("filter", t)
	}

	body, linkHeader, status, err := c.do(ctx, path+"?"+q.Encode(), path)
	if err != nil {
		return ListResult{}, err
	}
	if status != http.StatusOK {
		return ListResult{}, fmt.Errorf("huggingface: %s status %d: %s", path, status, truncate(body, 200))
	}
	var models []Model
	if err := json.Unmarshal(body, &models); err != nil {
		return ListResult{}, fmt.Errorf("decode %s: %w", path, err)
	}
	return ListResult{Models: models, NextLink: parseNextLink(linkHeader.Get("Link"))}, nil
}

// SpacesTagsByType is an alias of SpaceTags, kept for semantic clarity.
// Deprecated: use SpaceTags instead.
func (c *Client) SpacesTagsByType(ctx context.Context) (TagGroups, error) {
	return c.SpaceTags(ctx)
}

// parseNextLink extracts the cursor for rel="next" from an RFC 5988 Link header.
// Returns "" when no next link is present. HF encodes the next cursor either as
// the cursor value itself or as a URL whose query string already contains
// cursor=...; we return whatever HF would accept on the next call.
func parseNextLink(h string) string {
	for _, part := range strings.Split(h, ",") {
		p := strings.TrimSpace(part)
		if !strings.Contains(p, `rel="next"`) {
			continue
		}
		open := strings.Index(p, "<")
		close := strings.Index(p, ">")
		if open < 0 || close <= open {
			continue
		}
		href := p[open+1 : close]
		if u, err := url.Parse(href); err == nil {
			if c := u.Query().Get("cursor"); c != "" {
				return c
			}
		}
		return href
	}
	return ""
}

// truncate shortens a byte slice for error messages.
func truncate(b []byte, n int) string {
	if len(b) <= n {
		return string(b)
	}
	return string(b[:n]) + "…"
}

// BoolFlag is a tiny helper for parsing optional bool form values.
func BoolFlag(s string) bool {
	b, _ := strconv.ParseBool(s)
	return b
}
