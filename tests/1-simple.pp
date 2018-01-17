#define(h){Hello};
#define(w){world};
Hello world! == #h #w!
#define(x){#h #w};
Hello world = #x
#define(z :y){#y world};
Hello world Hello == #z(Hello) #h
#define(q :y){#h #expand(#rescope(#y){#w} ) #w };
#define(w){new world order};
Hello world == #x
Hello new world order world == #q{ } x
