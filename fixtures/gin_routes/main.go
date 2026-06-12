package main

func health() string {
	return "ok"
}

func main() {
	r := ginDefault()
	r.GET("/health", health)
}

func ginDefault() *ginEngine {
	return &ginEngine{}
}

type ginEngine struct{}

func (r *ginEngine) GET(path string, handler func() string) {}
