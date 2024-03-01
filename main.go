package main

import "fmt"

func main() {
	config := LoadConfigFromFlags()

	fmt.Printf("Config: %+v\n", config)
}
