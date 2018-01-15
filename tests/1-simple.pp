#define(h){Hello};
#define(w){world};
Hello world! == #h #w!
#define(x){#h #w};
#define(x :y){#h #expand(#rescope(#y){#w})}
Hello world = #x
#define(h :y){#y world};
Hello world Hello == #h(Hello) #h
#define(w){new world order};
Hello world == #x
Hello new world order == #x{}
