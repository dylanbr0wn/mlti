package main

import "context"

type SchedularOpts struct {
	tasks        []*Task
	taskChan     chan *Task
	completed    chan *Task
	doneTasks    chan *Task
	maxProcesses int
	numCompleted int
	numCmds      int
}
type SchedularOptsFunc func(*SchedularOpts)

type Scheduler struct {
	SchedularOpts
}

func WithMaxProcesses(max int) SchedularOptsFunc {
	return func(o *SchedularOpts) {
		o.maxProcesses = max
	}
}

func WithCommands(commands []*Command) SchedularOptsFunc {
	return func(o *SchedularOpts) {
		o.numCmds = len(commands)
	}
}

func NewScheduler(opts ...SchedularOptsFunc) *Scheduler {
	schedulerOpts := SchedularOpts{}

	for _, opt := range opts {
		opt(&schedulerOpts)
	}

	schedulerOpts.taskChan = make(chan *Task, schedulerOpts.numCmds)
	schedulerOpts.completed = make(chan *Task, schedulerOpts.numCmds)
	schedulerOpts.doneTasks = make(chan *Task, schedulerOpts.numCmds)

	return &Scheduler{
		schedulerOpts,
	}
}

func (s *Scheduler) Start(ctx context.Context) {
	inflight := 0
	for {
		select {
		case <-ctx.Done():
			return
		case task := <-s.taskChan:
			if inflight < s.maxProcesses {

				go s.run(ctx, task)
				inflight++
			} else {
				s.tasks = append(s.tasks, task)
			}
		case completedTask := <-s.completed:
			s.doneTasks <- completedTask
			s.numCompleted++
			inflight--
			if len(s.tasks) > 0 {
				task := s.tasks[0]
				s.tasks = s.tasks[1:]
				go s.run(ctx, task)
				inflight++
			}
		}
		if s.numCompleted == cap(s.completed) {
			return
		}
	}
}

func (s *Scheduler) run(ctx context.Context, task *Task) {
	task.Run(ctx)
	s.completed <- task
}

func (s *Scheduler) Schedule(task *Task) {
	s.taskChan <- task
}

func (s *Scheduler) Completed() <-chan *Task {
	return s.doneTasks
}

func (s *Scheduler) Max() int {
	return s.maxProcesses
}
