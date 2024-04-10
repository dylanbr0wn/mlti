package main

var pastels = []Color{
	"#FFB3B3",
	"#C1EFFF",
	"#B5FFD6",
	"#FFDBA4",
	"#B5FFB9",
	"#FF7878",
	"#E0C097",
	"#FFE9AE",
}

type Pallete string

const (
	Pastel Pallete = "pastel"
)

type Color string

type ColorFactory struct {
	pallete []Color
	index   int
}

func NewColorFactory(pallete Pallete) *ColorFactory {
	switch {
	case pallete == Pastel:
		return &ColorFactory{
			pallete: pastels,
		}
	default:
		return &ColorFactory{
			pallete: pastels,
		}
	}
}

func (cf *ColorFactory) Generate() Color {
	color := cf.pallete[cf.index]
	cf.index = (cf.index + 1) % len(cf.pallete)
	return color
}
