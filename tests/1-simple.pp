#define(h){Hello};
#define(w){world};
#h #w!
#define(x){#h #w};
#x --
#define(h :x){#x world};
#h(Hello) #h
