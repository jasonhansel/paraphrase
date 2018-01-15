#define(h){Hello};
#define(w){world};
Hello world! == #h #w!
#define(x){#h #w};
Hello world = #x
#define(h :y){#y world};
Hello world Hello == #h(Hello) #h
#define(w){new world order};
Hello world == #x
#define(q :y){#h #expand(#rescope(#y){#w} ) #w };
Hello new world order world == #q{ } x
