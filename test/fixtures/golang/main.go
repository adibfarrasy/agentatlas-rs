package main

import "fmt"

type Point struct {
	X, Y int
}

func (p Point) Dist() int {
	return p.X + p.Y
}

func main() {
	p := Point{1, 2}
	fmt.Println(p.Dist())
}
