package main

import (
	"flag"
)

type FlagOptsFunc func(*FlagOpts)

type FlagOpts struct {
	name        string
	description string
	short       string
	required    bool
	defaultVal  interface{}
}

func Name(name string) FlagOptsFunc {
	return func(o *FlagOpts) {
		o.name = name
	}
}

func Description(description string) FlagOptsFunc {
	return func(o *FlagOpts) {
		o.description = description
	}
}

func Short(short string) FlagOptsFunc {
	return func(o *FlagOpts) {
		o.short = short
	}
}

func Required(o *FlagOpts) {
	o.required = true
}

func Default(val interface{}) FlagOptsFunc {
	return func(o *FlagOpts) {
		o.defaultVal = val
	}
}

type Flag struct {
	FlagOpts
}

type StringFlag struct {
	value      []*string
	defaultVal string
}

func (f *StringFlag) Get() string {
	if len(f.value) > 1 {
		if *f.value[1] == *f.value[0] {
			return *f.value[1]
		} else if *f.value[1] != f.defaultVal {
			return *f.value[1]
		} else {
			return *f.value[0]
		}
	}
	return *f.value[0]
}

type IntFlag struct {
	value      []*int
	defaultVal int
}

func (f *IntFlag) Get() int {
	if len(f.value) > 1 {
		if *f.value[1] == *f.value[0] {
			return *f.value[1]
		} else if *f.value[1] != f.defaultVal {
			return *f.value[1]
		} else {
			return *f.value[0]
		}
	}
	return *f.value[0]
}

type BoolFlag struct {
	value      []*bool
	defaultVal bool
}

func (f *BoolFlag) Get() bool {
	if len(f.value) > 1 {
		if *f.value[1] == *f.value[0] {
			return *f.value[1]
		} else if *f.value[1] != f.defaultVal {
			return *f.value[1]
		} else {
			return *f.value[0]
		}
	}
	return *f.value[0]
}

func NewFlag(opts ...FlagOptsFunc) *Flag {
	o := FlagOpts{}
	for _, opt := range opts {
		opt(&o)
	}
	return &Flag{FlagOpts: o}
}

func (f *Flag) String() *StringFlag {
	if f.defaultVal == nil {
		f.defaultVal = ""
	}
	values := []*string{
		flag.String(f.name, f.defaultVal.(string), f.description),
	}
	if f.short != "" {
		values = append(values, flag.String(f.short, f.defaultVal.(string), f.description))
	}
	return &StringFlag{value: values, defaultVal: f.defaultVal.(string)}
}
func (f *Flag) Int() *IntFlag {
	if f.defaultVal == nil {
		f.defaultVal = 0
	}
	values := []*int{
		flag.Int(f.name, f.defaultVal.(int), f.description),
	}
	if f.short != "" {
		values = append(values, flag.Int(f.short, f.defaultVal.(int), f.description))
	}

	return &IntFlag{value: values, defaultVal: f.defaultVal.(int)}
}
func (f *Flag) Bool() *BoolFlag {
	if f.defaultVal == nil {
		f.defaultVal = false
	}
	values := []*bool{
		flag.Bool(f.name, f.defaultVal.(bool), f.description),
	}
	if f.short != "" {
		values = append(values, flag.Bool(f.short, f.defaultVal.(bool), f.description))
	}
	return &BoolFlag{value: values, defaultVal: f.defaultVal.(bool)}
}

func ParseFlags() {
	flag.Parse()
}

func GetArgs() []string {
	return flag.Args()
}
